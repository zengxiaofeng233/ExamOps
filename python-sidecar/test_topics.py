import unittest
import chapters
import topics
from structure import Block, Unit


class TopicTests(unittest.TestCase):
    def sample(self, texts):
        return [Unit(i+1, str(i+1), [Block(text * 25, bbox=[50,200,550,700])], width=600, height=800) for i, text in enumerate(texts)]

    def test_lexical_transition_yields_virtual_units_and_preserves_body(self):
        units = self.sample(['galaxy planet orbit telescope gravity stellar. '] * 6
                            + ['enzyme protein cellular membrane biology genetic. '] * 6)
        original = [b.text for u in units for b in u.blocks]
        self.assertEqual(chapters.detect(units, 'pdf'), 'topic')
        headings = [(u.number, b) for u in units for b in u.blocks if b.heading]
        self.assertEqual([n for n, _ in headings], [1, 7])
        self.assertTrue(all(b.source == 'topic' and b.kind == 'synthetic' for _, b in headings))
        self.assertEqual([b.text for u in units for b in u.blocks if b.kind != 'synthetic'], original)

    def test_chinese_bigrams_find_transition_inside_a_page(self):
        units = [Unit(1, '1', [Block(text * 20) for text in
                 ['函数极限连续收敛数列变化。'] * 6 + ['细胞生物遗传染色体蛋白质。'] * 6])]
        self.assertEqual(topics.apply(units), 2)
        self.assertEqual(sum(b.heading is not None for b in units[0].blocks), 2)

    def test_uniform_short_formula_or_existing_title_never_gets_topic_units(self):
        for units in [self.sample(['galaxy planet orbit telescope gravity stellar. '] * 15),
                      self.sample(['galaxy planet. ', 'enzyme protein. ']),
                      [Unit(1, '1', [Block('∫₀¹ x² dx = α' * 1000)])]]:
            self.assertEqual(topics.apply(units), 0)
        units = self.sample(['galaxy planet orbit telescope gravity stellar. '] * 6
                            + ['enzyme protein cellular membrane biology genetic. '] * 6)
        units[0].blocks.insert(0, Block('第一章概述'))
        self.assertEqual(chapters.detect(units, 'pdf'), 'patterns')
        self.assertFalse(any(b.source == 'topic' for u in units for b in u.blocks))

    def test_split_chapter_title_is_joined_in_geometric_order(self):
        units = [Unit(1, '1', [Block('定积分的应用', kind='ocr', bbox=[110,700,220,720]),
                               Block('第六章', kind='ocr', bbox=[48,700,104,720]),
                               Block('这里是正文。', kind='ocr', bbox=[48,660,200,680])])]
        self.assertEqual(chapters.detect(units, 'pdf'), 'patterns')
        self.assertEqual(units[0].blocks[0].text, '第六章 定积分的应用')
        self.assertEqual(units[0].blocks[0].recognition, 'unverified')

    def test_running_chapter_number_and_references_are_not_new_sections(self):
        units = [Unit(1, '1', [Block('第一章 函数'), Block('第一节函数'), Block('第二节极限')]),
                 Unit(2, '2', [Block('第一章'), Block('第一节研究了函数，现在讨论极限'), Block('第三节连续')])]
        self.assertEqual(chapters.detect(units, 'pdf'), 'patterns')
        self.assertEqual([b.text for u in units for b in u.blocks if b.heading],
                         ['第一章 函数', '第一节函数', '第二节极限', '第三节连续'])

    def test_bookmark_chapters_scope_repeated_ocr_section_numbers(self):
        units = [Unit(1, '1', [Block('第一章 函数'), Block('第一节 定义', kind='ocr')]),
                 Unit(2, '2', [Block('第二章 导数'), Block('第一节 定义', kind='ocr')])]
        self.assertEqual(chapters.detect(units, 'pdf', [(0, '第一章 函数', 0), (0, '第二章 导数', 1)]), 'mixed')
        self.assertEqual([b.heading for u in units for b in u.blocks], [1, 2, 1, 2])

    def test_a_real_math_heading_keeps_its_symbols(self):
        units = [Unit(1, '1', [Block('第一节 L²空间')])]
        self.assertEqual(chapters.detect(units, 'pdf'), 'patterns')
        self.assertEqual(units[0].blocks[0].text, '第一节 L²空间')
        self.assertEqual(units[0].blocks[0].heading, 2)


if __name__ == '__main__':
    unittest.main()

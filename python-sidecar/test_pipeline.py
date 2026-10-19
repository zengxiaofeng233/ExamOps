import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from PIL import Image
from PyPDF2 import PdfReader, PdfWriter
from PyPDF2.generic import RectangleObject

from checkpoints import Checkpoints
from ocr_runtime import worker_limit, GIB
from parser import Ocr, parse_structured
from test_parser import make_pdf


class PipelineTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.path = self.root / 'test.pdf'

    def tearDown(self):
        self.tmp.cleanup()

    def test_mixed_page_uses_region_and_retains_canvas_coordinates(self):
        make_pdf(self.path, Image.new('RGB', (520, 130), 'white'))
        seen = []
        def recognize(ocr, image):
            seen.append(image.size)
            return [([[10, 10], [50, 10], [50, 30], [10, 30]], 'Image words')]
        with patch.object(Ocr, 'lines', recognize):
            result = parse_structured(self.path, ocr_mode='cpu')
        self.assertEqual(len(seen), 1)
        self.assertLess(seen[0][1], 500)  # original full page was 2000 pixels high
        self.assertIn('Native title', result['content'])
        self.assertIn('Image words', result['content'])
        box = next(b['bbox'] for b in result['pages'][0]['blocks'] if b['kind'] == 'ocr')
        self.assertGreaterEqual(box[0], 38)
        self.assertLessEqual(box[3], 632)
        self.assertGreaterEqual(box[1], 498)

    def test_rotated_cropped_region_preserves_pdf_coordinates(self):
        make_pdf(self.path, Image.new('RGB', (520, 130), 'white'))
        reader = PdfReader(self.path)
        page = reader.pages[0]
        page.rotate(90)
        page.cropbox = RectangleObject([20, 100, 580, 780])
        writer = PdfWriter(); writer.add_page(page)
        rotated = self.root / 'rotated.pdf'
        with rotated.open('wb') as stream: writer.write(stream)
        with patch.object(Ocr, 'lines', return_value=[([[10,10],[30,10],[30,50],[10,50]], 'Rotated')]):
            result = parse_structured(rotated, ocr_mode='cpu')
        box = next(b['bbox'] for b in result['pages'][0]['blocks'] if b['kind'] == 'ocr')
        self.assertTrue(38 <= box[0] < box[2] <= 562, box)
        self.assertTrue(498 <= box[1] < box[3] <= 632, box)

    def test_empty_high_resolution_retry_keeps_first_recognition(self):
        make_pdf(self.path, Image.new('RGB', (520,130), 'white'))
        calls = []
        def recognize(ocr, image):
            calls.append(image.size)
            ocr.confidence = .6
            return [([[10,10],[50,10],[50,30],[10,30]],'Keep this text')] if len(calls) == 1 else []
        with patch.object(Ocr, 'lines', recognize):
            result = parse_structured(self.path, ocr_mode='cpu')
        self.assertEqual(len(calls),2)
        self.assertGreater(calls[1][0],calls[0][0])
        self.assertIn('Keep this text',result['content'])

    def test_interruption_reuses_completed_pages_and_streams_them_again(self):
        make_pdf(self.path, Image.new('RGB', (520,130), 'white'), pages=3)
        cache = self.root / 'cache'
        class Interrupt:
            def __call__(self, *args): pass
            def page(self, unit, images, total):
                if unit.number == 2: raise RuntimeError('interrupted')
        recognized = [([[10,10],[50,10],[50,30],[10,30]], 'Cached words')]
        with patch('pdf_pipeline.worker_limit', return_value=1), patch.object(Ocr, 'lines', return_value=recognized) as ocr:
            with self.assertRaisesRegex(RuntimeError, 'interrupted'):
                parse_structured(self.path, progress=Interrupt(), cache_dir=cache, ocr_mode='cpu')
            self.assertEqual(ocr.call_count, 2)
        seen = []
        class Observe:
            def __call__(self, *args): pass
            def page(self, unit, images, total): seen.append(unit.number)
        with patch('pdf_pipeline.worker_limit', return_value=1), patch.object(Ocr, 'lines', return_value=recognized) as ocr:
            result = parse_structured(self.path, progress=Observe(), cache_dir=cache, ocr_mode='cpu')
            self.assertEqual(ocr.call_count, 1)
        self.assertEqual(seen, [1,2,3])
        self.assertEqual(len(result['pages']),3)

    def test_corrupt_checkpoint_is_recomputed_and_source_change_invalidates_cache(self):
        make_pdf(self.path)
        cache = self.root / 'cache'
        with patch('pdf_pipeline.worker_limit', return_value=1):
            parse_structured(self.path, cache_dir=cache, ocr_mode='cpu')
            record = next(cache.glob('*/1.json'))
            record.write_text('{broken', encoding='utf-8')
            result = parse_structured(self.path, cache_dir=cache, ocr_mode='cpu')
            self.assertIn('Native title', result['content'])
            self.assertIsInstance(json.loads(record.read_text(encoding="utf-8")), dict)
            make_pdf(self.path, native='Changed title')
            result = parse_structured(self.path, cache_dir=cache, ocr_mode='cpu')
            self.assertIn('Changed title', result['content'])
            self.assertEqual(len(list(cache.glob('*/1.json'))), 2)

    def test_memory_limits_and_gpu_failure_fallback(self):
        self.assertEqual(worker_limit(available=GIB,total=16*GIB,cpus=16),1)
        self.assertEqual(worker_limit(available=24*GIB,total=32*GIB,cpus=16),4)
        self.assertEqual(worker_limit(available=24*GIB,total=32*GIB,cpus=2),1)
        class Result:
            txts = ['CPU result']; boxes = [[[0,0],[10,0],[10,10],[0,10]]]; scores = [1.0]
        with patch('ocr_runtime.provider_for',return_value='directml'), patch('ocr_runtime.make_engine',side_effect=[RuntimeError('GPU unavailable'),lambda image: Result()]):
            ocr = Ocr()
            self.assertEqual(ocr.lines(Image.new('RGB',(20,20)))[0][1],'CPU result')
            self.assertEqual(ocr.active_mode,'cpu')

    def test_spawn_workers_match_serial_page_order(self):
        from test_structure import make_text_pdf
        make_text_pdf(self.path, [[(text, 12)] for text in [
            'Functions map inputs to outputs.', 'Derivatives describe rates of change.',
            'Integrals describe accumulated quantities.', 'Matrices represent linear transformations.']])
        serial = parse_structured(self.path, ocr_mode='cpu')
        import psutil
        workers = []
        class Observe:
            def __call__(self, *args): pass
            def page(self, *args): workers.append(len(psutil.Process().children()))
        with patch('pdf_pipeline.worker_limit', return_value=2), patch('pdf_pipeline.available_worker_slots', return_value=2):
            parallel = parse_structured(self.path, cache_dir=self.root/'parallel', ocr_mode='cpu', progress=Observe())
        self.assertEqual(parallel, serial)
        self.assertGreaterEqual(max(workers), 2)


if __name__ == '__main__': unittest.main()

import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { Course, UpdateCourseInput } from '@/types/models'
import { CourseService } from '@/services/course'
import { errorMessage } from '@/utils/errors'

export const useCourseStore = defineStore('course', () => {
  const courses = ref<Course[]>([])
  const loaded = ref(false)
  const loading = ref(false)
  const error = ref<string | null>(null)

  const currentCourse = ref<Course | null>(null)

  const sortedCourses = computed(() => {
    return [...courses.value].sort((a, b) => b.updated_at.localeCompare(a.updated_at) || b.id - a.id)
  })

  function upsert(course: Course) {
    const index = courses.value.findIndex(c => c.id === course.id)
    if (index === -1) courses.value.push(course)
    else courses.value[index] = course
    if (currentCourse.value?.id === course.id) currentCourse.value = course
  }

  async function fetchCourses() {
    loading.value = true
    error.value = null
    try {
      courses.value = await CourseService.getCourses()
      loaded.value = true
    } catch (e) {
      error.value = errorMessage(e)
      throw e
    } finally {
      loading.value = false
    }
  }

  async function ensureLoaded() {
    if (!loaded.value) await fetchCourses().catch(() => undefined)
  }

  async function loadCourse(id: number) {
    const course = await CourseService.getCourse(id)
    upsert(course)
    currentCourse.value = course
    return course
  }

  async function createCourse(name: string, description: string) {
    const course = await CourseService.createCourse({ name, description })
    upsert(course)
    return course
  }

  async function updateCourse(input: UpdateCourseInput) {
    const updated = await CourseService.updateCourse(input)
    upsert(updated)
    return updated
  }

  async function deleteCourse(id: number) {
    await CourseService.deleteCourse(id)
    courses.value = courses.value.filter(c => c.id !== id)
    if (currentCourse.value?.id === id) currentCourse.value = null
  }

  /** Bumps a course to the top of "recent" after activity without a round trip. */
  function touch(id: number) {
    const course = courses.value.find(c => c.id === id)
    if (!course) return
    const now = new Date()
    const pad = (n: number) => String(n).padStart(2, '0')
    course.updated_at = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())} ${pad(now.getHours())}:${pad(now.getMinutes())}:${pad(now.getSeconds())}`
  }

  return {
    courses,
    sortedCourses,
    currentCourse,
    loading,
    loaded,
    error,
    fetchCourses,
    ensureLoaded,
    loadCourse,
    createCourse,
    updateCourse,
    deleteCourse,
    touch
  }
})

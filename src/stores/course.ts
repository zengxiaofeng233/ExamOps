import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { Course } from '@/types/models'
import { CourseService } from '@/services/course'

export const useCourseStore = defineStore('course', () => {
  const courses = ref<Course[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  const currentCourse = ref<Course | null>(null)

  const sortedCourses = computed(() => {
    return [...courses.value].sort((a, b) => {
      return new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime()
    })
  })

  async function fetchCourses() {
    loading.value = true
    error.value = null
    try {
      courses.value = await CourseService.getCourses()
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch courses'
      throw e
    } finally {
      loading.value = false
    }
  }

  async function createCourse(name: string, description: string) {
    loading.value = true
    error.value = null
    try {
      const course = await CourseService.createCourse({ name, description })
      courses.value.push(course)
      return course
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to create course'
      throw e
    } finally {
      loading.value = false
    }
  }

  async function updateCourse(id: number, name?: string, description?: string) {
    loading.value = true
    error.value = null
    try {
      const updated = await CourseService.updateCourse({ id, name, description })
      const index = courses.value.findIndex(c => c.id === id)
      if (index !== -1) {
        courses.value[index] = updated
      }
      return updated
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to update course'
      throw e
    } finally {
      loading.value = false
    }
  }

  async function deleteCourse(id: number) {
    loading.value = true
    error.value = null
    try {
      await CourseService.deleteCourse(id)
      courses.value = courses.value.filter(c => c.id !== id)
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to delete course'
      throw e
    } finally {
      loading.value = false
    }
  }

  function setCurrentCourse(course: Course | null) {
    currentCourse.value = course
  }

  return {
    courses,
    sortedCourses,
    currentCourse,
    loading,
    error,
    fetchCourses,
    createCourse,
    updateCourse,
    deleteCourse,
    setCurrentCourse
  }
})

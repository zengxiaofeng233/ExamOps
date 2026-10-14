import { invoke } from '@tauri-apps/api/core'
import type { Course, CreateCourseInput, UpdateCourseInput } from '@/types/models'

export const CourseService = {
  createCourse: (input: CreateCourseInput) => invoke<Course>('create_course', { input }),
  getCourses: () => invoke<Course[]>('get_courses'),
  getCourse: (id: number) => invoke<Course>('get_course', { id }),
  updateCourse: (input: UpdateCourseInput) => invoke<Course>('update_course', { input }),
  deleteCourse: (id: number) => invoke<void>('delete_course', { id })
}

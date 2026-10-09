import { invoke } from '@tauri-apps/api/core'
import type { Course, CreateCourseInput, UpdateCourseInput } from '@/types/models'

export class CourseService {
  static async createCourse(input: CreateCourseInput): Promise<Course> {
    return await invoke<Course>('create_course', { input })
  }

  static async getCourses(): Promise<Course[]> {
    return await invoke<Course[]>('get_courses')
  }

  static async updateCourse(input: UpdateCourseInput): Promise<Course> {
    return await invoke<Course>('update_course', { input })
  }

  static async deleteCourse(id: number): Promise<void> {
    return await invoke<void>('delete_course', { id })
  }
}

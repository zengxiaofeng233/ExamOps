import { Channel, invoke } from '@tauri-apps/api/core'
import type {
  ChatEvent, ContextMode, Conversation, ConversationDetail, ConversationScope, Message, ModelProfile,
  ModelListInput, ProfileInput, SaveProfileResult, TestReport, TutorMode, UsageSummary
} from '@/types/ai'
import type { LessonPart, StartTeachingInput, TeachAction } from '@/types/study'

export const ModelService = {
  list: () => invoke<ModelProfile[]>('list_model_profiles'),
  save: (input: ProfileInput) => invoke<SaveProfileResult>('save_model_profile', { input }),
  remove: (id: number) => invoke<void>('delete_model_profile', { id }),
  setDefault: (id: number) => invoke<void>('set_default_model_profile', { id }),
  test: (input: ProfileInput) => invoke<TestReport>('test_model_profile', { input }),
  fetchModels: (input: ModelListInput) => invoke<string[]>('fetch_available_models', { input }),
  usage: () => invoke<UsageSummary>('get_ai_usage')
}

export interface NewConversation {
  course_id: number
  mode?: TutorMode
  title?: string
  context_mode?: ContextMode
  scope?: ConversationScope
}

export interface ConversationUpdate {
  id: number
  title?: string
  context_mode?: ContextMode
  scope?: ConversationScope
  /** 0 = use the default model */
  profile_id?: number
}

/** Teaching action for learn / review / sprint conversations (default: a question). */
export interface SendOptions {
  action?: TeachAction
  target?: number
}

export const ChatService = {
  list: (courseId: number) => invoke<Conversation[]>('list_conversations', { courseId }),
  create: (input: NewConversation) => invoke<Conversation>('create_conversation', { input }),
  get: (id: number) => invoke<ConversationDetail>('get_conversation', { id }),
  update: (input: ConversationUpdate) => invoke<Conversation>('update_conversation', { input }),
  remove: (id: number) => invoke<void>('delete_conversation', { id }),
  send: (conversationId: number, content: string, onEvent: (event: ChatEvent) => void, regenerate = false, options: SendOptions = {}) => {
    const channel = new Channel<ChatEvent>()
    channel.onmessage = onEvent
    const input = { conversation_id: conversationId, content, regenerate, action: options.action ?? null, target: options.target ?? null }
    return invoke<Message>('send_message', { input, onEvent: channel })
  },
  cancel: (requestId: string) => invoke<boolean>('cancel_generation', { requestId })
}

export const TeachingService = {
  /** Opens (or creates) the teaching conversation for a chapter / video and mode. */
  start: (input: StartTeachingInput) => invoke<Conversation>('start_teaching', { input }),
  outline: (conversationId: number) => invoke<LessonPart[]>('get_lesson_outline', { conversationId }),
  markFinished: (conversationId: number) => invoke<Conversation>('mark_lesson_finished', { conversationId })
}

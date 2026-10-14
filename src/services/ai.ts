import { Channel, invoke } from '@tauri-apps/api/core'
import type {
  ChatEvent, ContextMode, Conversation, ConversationDetail, ConversationScope, Message, ModelProfile,
  ProfileInput, SaveProfileResult, TestReport, TutorMode, UsageSummary
} from '@/types/ai'

export const ModelService = {
  list: () => invoke<ModelProfile[]>('list_model_profiles'),
  save: (input: ProfileInput) => invoke<SaveProfileResult>('save_model_profile', { input }),
  remove: (id: number) => invoke<void>('delete_model_profile', { id }),
  setDefault: (id: number) => invoke<void>('set_default_model_profile', { id }),
  test: (input: ProfileInput) => invoke<TestReport>('test_model_profile', { input }),
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

export const ChatService = {
  list: (courseId: number) => invoke<Conversation[]>('list_conversations', { courseId }),
  create: (input: NewConversation) => invoke<Conversation>('create_conversation', { input }),
  get: (id: number) => invoke<ConversationDetail>('get_conversation', { id }),
  update: (input: ConversationUpdate) => invoke<Conversation>('update_conversation', { input }),
  remove: (id: number) => invoke<void>('delete_conversation', { id }),
  send: (conversationId: number, content: string, onEvent: (event: ChatEvent) => void, regenerate = false) => {
    const channel = new Channel<ChatEvent>()
    channel.onmessage = onEvent
    return invoke<Message>('send_message', { input: { conversation_id: conversationId, content, regenerate }, onEvent: channel })
  },
  cancel: (requestId: string) => invoke<boolean>('cancel_generation', { requestId })
}

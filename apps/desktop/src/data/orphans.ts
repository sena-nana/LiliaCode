import { ref } from "vue";
import { makeId } from "./shared";

/**
 * 「收集箱」：还没绑定到任何项目的 Session/Task。形状沿用 Task，projectId 为 null。
 */
export interface OrphanConversation {
  id: string;
  sessionId: string;
  title: string;
  createdAt: number;
}

export const ORPHAN_LIST = ref<OrphanConversation[]>([
  {
    id: "o-001",
    sessionId: "0192-zzzz-0001",
    title: "随手问问 Claude：tsconfig paths",
    createdAt: Date.now() - 1000 * 60 * 12,
  },
  {
    id: "o-002",
    sessionId: "0192-zzzz-0002",
    title: "整理 Yarn 4 workspaces 笔记",
    createdAt: Date.now() - 1000 * 60 * 60 * 6,
  },
]);

/**
 * 草稿：点了「新对话」但还没发出第一条消息的会话。不进侧栏，首条发送成功后 promote 进 ORPHAN_LIST。
 */
const DRAFTS = new Map<string, OrphanConversation>();

export function listOrphanConversations(): OrphanConversation[] {
  return ORPHAN_LIST.value;
}

export function getOrphanConversation(id: string): OrphanConversation | undefined {
  return DRAFTS.get(id) ?? ORPHAN_LIST.value.find((o) => o.id === id);
}

export function isDraftOrphan(id: string): boolean {
  return DRAFTS.has(id);
}

/** 点「新对话」时调用：产出一条只活在内存里的草稿。 */
export function createDraftOrphan(): OrphanConversation {
  const id = makeId("o-draft");
  const draft: OrphanConversation = {
    id,
    sessionId: id,
    title: "新对话",
    createdAt: Date.now(),
  };
  DRAFTS.set(id, draft);
  return draft;
}

/**
 * 草稿发出第一条消息后调用：从 DRAFTS 移到 ORPHAN_LIST，title 用首条消息预览代替占位。
 */
export function promoteDraftOrphan(id: string, title: string): void {
  const draft = DRAFTS.get(id);
  if (!draft) return;
  DRAFTS.delete(id);
  if (ORPHAN_LIST.value.some((o) => o.id === id)) return;
  ORPHAN_LIST.value = [
    {
      ...draft,
      title: title || draft.title,
      createdAt: Date.now(),
    },
    ...ORPHAN_LIST.value,
  ];
}

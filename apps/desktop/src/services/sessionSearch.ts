/**
 * 会话搜索：按标题搜，覆盖项目下任务 + 收集箱对话。
 *
 * 三种模式：
 * - text   纯子串匹配（大小写不敏感），顺便算出命中区间给 UI 高亮。
 * - vector 字符 bigram TF-IDF + 余弦相似度。中英混排都能给有意义的相似度排序，
 *          零依赖。后续接 Anthropic/OpenAI embedding 时只需替换 `vectorize` 内部。
 * - hybrid 同时跑 text + vector，按 route 合并；归一化方式见 `searchHybrid`。
 *
 * 当前只搜标题——消息体存在内存 HashMap 重启就丢，等持久化落盘后再扩到 content。
 */

import { listProjects } from "./projectsStore";
import {
  listOrphanConversations,
  listProjectConversations,
} from "./tasksStore";

export type SearchMode = "text" | "vector" | "hybrid";

export type SearchKind = "project-task" | "orphan";

/** 标识结果在 hybrid 模式下来自哪一路：纯文本子串、向量相似度，或同时被两边命中。 */
export type SearchSource = "text" | "vector" | "both";

export interface SearchResult {
  kind: SearchKind;
  /** 项目任务才有；orphan 时为 undefined。 */
  projectId?: string;
  /** 项目展示名；orphan 时为 undefined（UI 显示「收集箱」标签）。 */
  projectName?: string;
  taskId: string;
  title: string;
  /** 直接可以 router.push 的路径。 */
  route: string;
  /** 越大越相关。不同模式的量纲不同，UI 只用来排序。 */
  score: number;
  /** title 上需要高亮的 [start, end) 区间。纯向量命中时为空。 */
  highlights: Array<[number, number]>;
  /** 命中来源；text/vector 模式恒等于自身，hybrid 模式按实际命中路决定。 */
  source: SearchSource;
}

interface Doc {
  kind: SearchKind;
  projectId?: string;
  projectName?: string;
  taskId: string;
  title: string;
  route: string;
}

function buildCorpus(): Doc[] {
  const docs: Doc[] = [];
  for (const p of listProjects()) {
    for (const t of listProjectConversations(p.id)) {
      docs.push({
        kind: "project-task",
        projectId: p.id,
        projectName: p.name,
        taskId: t.id,
        title: t.title,
        route: `/projects/${p.id}/tasks/${t.id}`,
      });
    }
  }
  for (const o of listOrphanConversations()) {
    docs.push({
      kind: "orphan",
      taskId: o.id,
      title: o.title,
      route: `/chats/${o.id}`,
    });
  }
  return docs;
}

// ---------------- text mode ----------------

/** 同一查询在标题里所有命中点的 [start, end) 区间。 */
function findRanges(title: string, query: string): Array<[number, number]> {
  const t = title.toLowerCase();
  const q = query.toLowerCase().trim();
  if (!q) return [];
  const ranges: Array<[number, number]> = [];
  let idx = 0;
  while ((idx = t.indexOf(q, idx)) !== -1) {
    ranges.push([idx, idx + q.length]);
    idx += q.length;
  }
  if (ranges.length > 0) return ranges;
  // 整串搜不到 → 按 whitespace 拆 token 再分别找。
  for (const tok of q.split(/\s+/).filter(Boolean)) {
    let i = 0;
    while ((i = t.indexOf(tok, i)) !== -1) {
      ranges.push([i, i + tok.length]);
      i += tok.length;
    }
  }
  return ranges;
}

function searchText(query: string, corpus: Doc[]): SearchResult[] {
  const q = query.trim();
  if (!q) return [];
  const out: SearchResult[] = [];
  for (const d of corpus) {
    const ranges = findRanges(d.title, q);
    if (ranges.length === 0) continue;
    const earliest = Math.min(...ranges.map((r) => r[0]));
    // 越多命中 / 越靠前得分越高；常数 10 让命中数主导。
    const score =
      ranges.length * 10 + (1 - earliest / Math.max(d.title.length, 1));
    out.push({ ...d, score, highlights: ranges, source: "text" });
  }
  return out.sort((a, b) => b.score - a.score);
}

// ---------------- vector mode ----------------

function normalize(s: string): string {
  return s.toLowerCase().replace(/\s+/g, " ").trim();
}

function bigrams(s: string): string[] {
  const norm = normalize(s);
  if (norm.length === 0) return [];
  if (norm.length === 1) return [norm];
  const out: string[] = [];
  for (let i = 0; i < norm.length - 1; i++) {
    out.push(norm.slice(i, i + 2));
  }
  return out;
}

function termFreq(tokens: string[]): Map<string, number> {
  const tf = new Map<string, number>();
  for (const t of tokens) tf.set(t, (tf.get(t) ?? 0) + 1);
  return tf;
}

function buildIdf(docs: Doc[]): Map<string, number> {
  const n = docs.length || 1;
  const df = new Map<string, number>();
  for (const d of docs) {
    const seen = new Set(bigrams(d.title));
    for (const t of seen) df.set(t, (df.get(t) ?? 0) + 1);
  }
  const idf = new Map<string, number>();
  for (const [t, c] of df) idf.set(t, Math.log(1 + n / c));
  return idf;
}

function tfidfVec(tokens: string[], idf: Map<string, number>): Map<string, number> {
  const tf = termFreq(tokens);
  const total = tokens.length || 1;
  const v = new Map<string, number>();
  for (const [t, c] of tf) {
    const w = (c / total) * (idf.get(t) ?? 0);
    if (w > 0) v.set(t, w);
  }
  return v;
}

function cosine(a: Map<string, number>, b: Map<string, number>): number {
  const [small, large] = a.size <= b.size ? [a, b] : [b, a];
  let dot = 0;
  for (const [k, va] of small) {
    const vb = large.get(k);
    if (vb !== undefined) dot += va * vb;
  }
  if (dot === 0) return 0;
  let na = 0;
  let nb = 0;
  for (const v of a.values()) na += v * v;
  for (const v of b.values()) nb += v * v;
  if (na === 0 || nb === 0) return 0;
  return dot / (Math.sqrt(na) * Math.sqrt(nb));
}

function searchVector(query: string, corpus: Doc[]): SearchResult[] {
  const q = query.trim();
  if (!q) return [];
  if (corpus.length === 0) return [];
  const idf = buildIdf(corpus);
  const qVec = tfidfVec(bigrams(q), idf);
  if (qVec.size === 0) return [];
  const out: SearchResult[] = [];
  for (const d of corpus) {
    const dVec = tfidfVec(bigrams(d.title), idf);
    const score = cosine(qVec, dVec);
    if (score <= 0) continue;
    out.push({ ...d, score, highlights: [], source: "vector" });
  }
  return out.sort((a, b) => b.score - a.score);
}

// ---------------- hybrid mode ----------------

/** 合并两路结果：text 分数除以最高 text 分归一到 [0,1]；权重 0.6/0.4 略偏向文本。 */
function searchHybrid(query: string, corpus: Doc[]): SearchResult[] {
  const q = query.trim();
  if (!q) return [];
  const textHits = searchText(q, corpus);
  const vectorHits = searchVector(q, corpus);

  const TEXT_WEIGHT = 0.6;
  const VECTOR_WEIGHT = 0.4;

  const textMax = textHits.length ? textHits[0].score : 0;
  const norm = (s: number) => (textMax > 0 ? s / textMax : 0);

  const merged = new Map<string, SearchResult>();

  for (const r of textHits) {
    merged.set(r.route, {
      ...r,
      score: norm(r.score) * TEXT_WEIGHT,
      source: "text",
    });
  }

  for (const r of vectorHits) {
    const prev = merged.get(r.route);
    if (prev) {
      merged.set(r.route, {
        ...prev,
        score: prev.score + r.score * VECTOR_WEIGHT,
        source: "both",
      });
    } else {
      merged.set(r.route, {
        ...r,
        score: r.score * VECTOR_WEIGHT,
        source: "vector",
      });
    }
  }

  return Array.from(merged.values()).sort((a, b) => b.score - a.score);
}

// ---------------- public ----------------

export function searchSessions(query: string, mode: SearchMode): SearchResult[] {
  const corpus = buildCorpus();
  if (mode === "vector") return searchVector(query, corpus);
  if (mode === "hybrid") return searchHybrid(query, corpus);
  return searchText(query, corpus);
}

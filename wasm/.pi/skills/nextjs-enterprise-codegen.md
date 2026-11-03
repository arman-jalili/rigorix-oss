---
name: nextjs-enterprise-codegen
description: "Full reference for Next.js enterprise code generation with DDD + Clean Architecture. Every module uses 4 DDD layers: domain/, application/, infrastructure/, interfaces/. Covers User Intents, App Shell, design system, navigation philosophy, offline strategy, server/client boundary, and testing. Loaded on-demand via agents/nextjs-codegen.md skill."
---

# Next.js Enterprise Code Generation — DDD + Clean Architecture

> Canonical skill for generating production-grade Next.js code with DDD patterns.
> **Every module MUST follow the 4 DDD layers below.**
> Validators enforce compliance.
>
> For design system and styling patterns, see: `.pi/skills/design-system-enterprise-codegen.md`

---

## 1. DDD Module Structure — The 4 Layers

Every feature module uses exactly this structure:

```
src/
  collaboration/                       # One bounded context / feature
    domain/                            # Layer 1: Pure business logic
      types.ts           #   Types, interfaces, enums (zero React)
      errors.ts          #   Typed error classes
      notification.ts    #   Domain value objects
    application/                       # Layer 2: State + use cases
      store.ts           #   Zustand store
      actions.ts         #   Server Actions
    infrastructure/                    # Layer 3: External adapters
      api.ts             #   API client (fetch)
    interfaces/                        # Layer 4: UI components
      thread-panel.tsx                 #   React components
      comment-list.tsx
```

### Dependency Rule (Inward)

```
domain → application → infrastructure → interfaces
```

- `domain/` — **ZERO React/Next.js imports.** Pure TypeScript. No `'use client'`.
- `application/` — imports from `domain/`. Stores, actions, server actions.
- `infrastructure/` — imports from `domain/` and `application/`. API clients, SDK wrappers.
- `interfaces/` — imports from `application/`. **Only this layer has React components.**

### What goes where — specific rules

| DO put in `domain/` | DO NOT put in `domain/` |
|--------------------|------------------------|
| TypeScript interfaces and types | React components |
| Error classes (`class NotFoundError`) | Zustand stores |
| Value objects (`class Money`) | `useState`, `useEffect` |
| Enum definitions (`enum Status`) | Server Actions (`'use server'`) |
| Pure functions (no side effects) | API fetch calls |
| Constants and configuration | CSS, Tailwind classes |

| DO put in `application/` | DO NOT put in `application/` |
|-------------------------|------------------------------|
| Zustand stores | React components |
| Server Actions (`'use server'`) | API fetch implementation |
| React Query hooks | UI state like `isOpen`, `isVisible` |
| Command/Query DTOs | JSX markup |

| DO put in `infrastructure/` | DO NOT put in `infrastructure/` |
|----------------------------|--------------------------------|
| API client functions (`fetch`, `axios`) | React components |
| localStorage/cookie wrappers | Store logic |
| Auth provider SDK wrappers | Domain types (import them) |
| Analytics SDK wrappers | UI state |

| DO put in `interfaces/` | DO NOT put in `interfaces/` |
|------------------------|-----------------------------|
| Page components (`page.tsx`) | API fetch calls |
| Client components (`*.client.tsx`) | Domain types (import them) |
| Layout components | Business logic |
| UI state (`useState` for modals) | Server Actions (call them, don't define) |

---

## 2. Domain Layer — Pure Business Logic

The domain layer has zero framework dependencies. It defines the types, errors, and value objects that the rest of the module uses.

```typescript
// collaboration/domain/types.ts
export interface Thread {
  id: string;
  knowledgeNodeId: string;
  title: string;
  createdAt: string;
  createdBy: User;
  replyCount: number;
  lastActivityAt: string;
}

export interface Comment {
  id: string;
  threadId: string;
  body: string;
  mentions: string[];
  createdAt: string;
  createdBy: User;
}
```

```typescript
// collaboration/domain/errors.ts
export class CommentError extends Error {
  constructor(
    public readonly code: string,
    message: string,
    public readonly retriable: boolean = false,
  ) {
    super(message);
    this.name = 'CommentError';
  }

  static saveFailed(): CommentError {
    return new CommentError('SAVE_FAILED', 'Failed to save comment', true);
  }

  static notFound(id: string): CommentError {
    return new CommentError('NOT_FOUND', `Comment ${id} not found`);
  }
}
```

---

## 3. Application Layer — Stores and Actions

```typescript
// collaboration/application/store.ts
import { create } from 'zustand';
import type { Thread, Comment } from '../domain/types';

interface CollaborationState {
  threads: Thread[];
  activeThreadId: string | null;
  isLoading: boolean;
  error: string | null;
  
  setThreads: (threads: Thread[]) => void;
  setActiveThread: (id: string) => void;
  optimisticallyAddComment: (comment: Comment) => void;
  rollbackComment: (commentId: string) => void;
}

export const useCollaborationStore = create<CollaborationState>((set) => ({
  threads: [],
  activeThreadId: null,
  isLoading: false,
  error: null,
  
  setThreads: (threads) => set({ threads }),
  setActiveThread: (id) => set({ activeThreadId: id }),
  optimisticallyAddComment: (comment) => set((state) => ({
    threads: state.threads.map(t =>
      t.id === comment.threadId
        ? { ...t, replyCount: t.replyCount + 1 }
        : t
    ),
  })),
  rollbackComment: (commentId) => set((state) => ({
    threads: state.threads.map(t =>
      t.id === commentId
        ? { ...t, replyCount: t.replyCount - 1 }
        : t
    ),
  })),
}));
```

```typescript
// collaboration/application/actions.ts
'use server';
import { revalidatePath } from 'next/cache';
import type { Comment } from '../domain/types';

export async function createComment(
  nodeId: string,
  body: string,
  mentions: string[],
): Promise<Comment> {
  // Server Action — calls backend API
  const res = await fetch(`/api/nodes/${nodeId}/comments`, {
    method: 'POST',
    body: JSON.stringify({ body, mentions }),
  });
  if (!res.ok) throw new Error('Failed to create comment');
  const comment = await res.json();
  revalidatePath(`/nodes/${nodeId}`);
  return comment;
}
```

---

## 4. Infrastructure Layer — API Clients

```typescript
// collaboration/infrastructure/api.ts
import type { Thread, Comment } from '../domain/types';
import { CommentError } from '../domain/errors';

export async function fetchThreads(nodeId: string): Promise<Thread[]> {
  const res = await fetch(`/api/nodes/${nodeId}/threads`);
  if (!res.ok) throw new CommentError('FETCH_FAILED', 'Failed to load threads', true);
  return res.json();
}

export async function saveComment(
  nodeId: string,
  threadId: string,
  body: string,
): Promise<Comment> {
  const res = await fetch(`/api/nodes/${nodeId}/threads/${threadId}/comments`, {
    method: 'POST',
    body: JSON.stringify({ body }),
  });
  if (!res.ok) throw CommentError.saveFailed();
  return res.json();
}
```

---

## 5. Interfaces Layer — React Components

```typescript
// collaboration/interfaces/thread-panel.tsx
'use client';
import { useCollaborationStore } from '../application/store';
import { fetchThreads } from '../infrastructure/api';
import { CommentList } from './comment-list';

export function ThreadPanel({ nodeId }: { nodeId: string }) {
  const { threads, activeThreadId, setActiveThread, setThreads } = useCollaborationStore();

  useEffect(() => {
    fetchThreads(nodeId).then(setThreads);
  }, [nodeId]);

  return (
    <div>
      {threads.map(thread => (
        <button key={thread.id} onClick={() => setActiveThread(thread.id)}>
          {thread.title} ({thread.replyCount})
        </button>
      ))}
      {activeThreadId && <CommentList threadId={activeThreadId} />}
    </div>
  );
}
```

---

## 6. Server/Client Boundary

| Layer | 'use client'? | 'use server'? | Can use hooks? |
|-------|--------------|---------------|----------------|
| `domain/` | No | No | No |
| `application/` | No (except stores) | Yes (actions) | No |
| `infrastructure/` | No | No | No |
| `interfaces/` | Yes | No | Yes |

---

## 7. Shared Code — When Something Spans Multiple Modules

Not everything belongs inside a module. The `shared/` directory holds cross-cutting code that multiple modules need. But it has strict rules.

### What goes in `shared/` vs `module/`

```
src/
  shared/                            # Cross-cutting, NOT a bounded context
    domain/                          # Shared domain concepts (Anti-Corruption Layer)
      user.ts                        #   User type used by multiple contexts
      team.ts                        #   Team type used by multiple contexts
      pagination.ts                  #   Generic pagination types
    hooks/                           # Generic React hooks (zero business logic)
      use-debounce.ts
      use-media-query.ts
      use-intersection-observer.ts
    lib/                             # Pure utilities
      date-utils.ts                  #   Date formatting
      string-utils.ts                #   Slug generation
      api-client.ts                  #   Base fetch wrapper (not domain-specific)
    ui/                              # Design system (framework-agnostic)
      button/
      card/
      modal/
  collaboration/                     # Bounded context
    domain/
      thread.ts                      #   Business-specific concept
      comment.ts
    application/
      store.ts
    infrastructure/
      api.ts
    interfaces/
      thread-panel.tsx
```

### The rules

| Goes in `shared/` | Goes in `module/` |
|------------------|-------------------|
| `useDebounce` (generic hook) | `useCollaborationStore` (domain-specific) |
| `apiClient.fetch()` (generic HTTP) | `collaborationApi.fetchThreads()` (domain-specific) |
| `formatDate()` (utility) | `calculateThreadActivity()` (domain logic) |
| `Button`, `Card` (design system) | `ThreadPanel`, `CommentList` (domain components) |
| `User`, `Team` (shared kernel) | `Thread`, `Comment` (domain entities) |

### The critical rule: `shared/` must NOT contain business logic

```typescript
// ❌ BAD — business logic in shared/
shared/lib/thread-utils.ts
  export function calculateThreadPriority(thread: Thread): number { }

// ✅ GOOD — business logic in the module
collaboration/domain/thread.ts
  export function calculateThreadPriority(thread: Thread): number { }
```

```typescript
// ✅ GOOD — generic infrastructure in shared/
shared/lib/api-client.ts
  export class ApiClient {
    async get<T>(path: string): Promise<T> { }
  }

// ✅ GOOD — domain-specific API in module infrastructure
collaboration/infrastructure/api.ts
  import { ApiClient } from '@/shared/lib/api-client';
  const api = new ApiClient();
  export async function fetchThreads(nodeId: string): Promise<Thread[]> {
    return api.get(`/nodes/${nodeId}/threads`);
  }
```

### When the same concept appears in multiple domains (Shared Kernel)

```typescript
// shared/domain/user.ts — User is a shared concept
// Shared kernel: multiple domains reference the same User
export interface User {
  id: string;
  name: string;
  email: string;
}

// collaboration/domain/thread.ts — imports shared User, doesn't redefine it
import { User } from '@/shared/domain/user';
export interface Thread {
  id: string;
  title: string;
  createdBy: User;  // References shared type, not duplicated
}
```

### File naming convention

Files are named by their **concept**, not prefixed with the module name:

| Wrong | Right |
|-------|-------|
| `collaboration-types.ts` | `types.ts` (or `thread.ts`, `comment.ts`) |
| `collaboration-errors.ts` | `errors.ts` |
| `collaboration-store.ts` | `store.ts` |

The directory path `collaboration/domain/thread.ts` already tells you which module it belongs to.

---

## 8. Testing by Layer

| Layer | Test type | Framework | What to test |
|-------|-----------|-----------|-------------|
| `domain/` | Unit | Vitest | Types, errors, pure functions |
| `application/` | Unit | Vitest | Store logic, action validation |
| `infrastructure/` | Integration | Vitest + MSW | API calls with mocked server |
| `interfaces/` | Component | Vitest + Testing Library | Render, user events, accessibility |

---

*Version: 1.2.0*
*Last updated: 2026-07-03*

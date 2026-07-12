---
name: typescript-enterprise-codegen
description: Full reference for TypeScript enterprise code generation with DDD + Clean Architecture. Covers module structure, domain entities, error classes, Express/Fastify controllers, TypeORM repositories, testing, and anti-patterns. Loaded on-demand via agents/typescript-codegen.md skill.
---

# TypeScript/Node.js Enterprise Code Generation — DDD + Clean Architecture

> Canonical skill for generating production-grade TypeScript code following Clean Architecture with DDD.
> All code MUST follow these patterns. Validators enforce compliance.
>
> Source: Clean Architecture principles + TypeScript best practices + DDD patterns from context7.

---

## 1. Project Structure — Clean Architecture with DDD

Every bounded context follows the same 4-layer structure:

```
module/
├── domain/                     # Pure domain entities, value objects, events
│   ├── index.ts                # Re-exports
│   ├── entity.ts               # Aggregate roots and entities
│   ├── value.ts                # Value objects
│   ├── event.ts                # Domain event payloads
│   ├── repository.ts           # Repository interfaces
│   └── errors.ts               # Typed error classes
├── application/                # Service interfaces and DTOs
│   ├── service.ts              # Service interface definitions
│   └── dto.ts                  # Input/Output DTOs with validation
├── infrastructure/             # Repository implementations, external adapters
│   ├── repository/             # Repository implementations
│   │   ├── typeorm-repo.ts
│   │   └── in-memory-repo.ts
│   └── persistence/            # Database connection, migrations
└── interfaces/                 # API contracts (HTTP)
    ├── controller.ts           # Route handler
    ├── middleware.ts            # Auth, validation middleware
    └── dto.ts                  # Request/Response schemas
```

### Dependency Direction Rule (Inward Dependency)

```
domain → application → infrastructure → interfaces
         ↑                    ↑
         └── inner layers never depend on outer layers
```

- **domain/** — depends on nothing except TS stdlib
- **application/** — depends on domain
- **infrastructure/** — depends on domain + application
- **interfaces/** — depends on application

---

## 2. Domain Layer — Entities & Value Objects

### Entity with Encapsulated State

```typescript
// entity.ts — Domain entity with identity.

import { randomUUID } from 'node:crypto';

export type Status = 'pending' | 'active' | 'completed' | 'failed';

export class Entity {
  private readonly _id: string;
  private _status: Status;
  private readonly _createdAt: Date;
  private _updatedAt: Date;

  constructor(id?: string) {
    this._id = id ?? randomUUID();
    this._status = 'pending';
    this._createdAt = new Date();
    this._updatedAt = new Date();
  }

  get id(): string { return this._id; }
  get status(): Status { return this._status; }
  get createdAt(): Date { return this._createdAt; }

  /** Transition state — throws on invalid transition. */
  transition(newStatus: Status): void {
    if (!this.canTransitionTo(newStatus)) {
      throw new DomainError(
        ErrorCode.InvalidTransition,
        `Cannot transition from ${this._status} to ${newStatus}`,
      );
    }
    this._status = newStatus;
    this._updatedAt = new Date();
  }

  private canTransitionTo(target: Status): boolean {
    const validTransitions: Record<Status, Status[]> = {
      pending: ['active'],
      active: ['completed', 'failed'],
      completed: [],
      failed: [],
    };
    return validTransitions[this._status].includes(target);
  }
}
```

### Value Object

```typescript
// value.ts — Immutable value object.

export class Money {
  public static readonly ZERO = new Money(0, 'USD');

  private constructor(
    private readonly _amount: number,
    private readonly _currency: string,
  ) { Object.freeze(this); }

  static create(amount: number, currency: string): Money {
    if (amount < 0) throw new DomainError(ErrorCode.Validation, 'Amount must be non-negative');
    if (currency.length !== 3) throw new DomainError(ErrorCode.Validation, 'Currency must be ISO 4217');
    return new Money(Math.round(amount * 100), currency.toUpperCase());
  }

  get amount(): number { return this._amount; }
  get currency(): string { return this._currency; }

  add(other: Money): Money {
    if (this._currency !== other._currency) {
      throw new DomainError(ErrorCode.Validation, 'Currency mismatch');
    }
    return new Money(this._amount + other._amount, this._currency);
  }

  equals(other: unknown): boolean {
    return other instanceof Money &&
      this._amount === other._amount &&
      this._currency === other._currency;
  }
}
```

### Repository Interface

```typescript
// repository.ts — Interface defined in domain, implemented in infrastructure.

export interface Repository<T extends Entity> {
  findById(id: string): Promise<T | null>;
  save(entity: T): Promise<void>;
  delete(id: string): Promise<void>;
  findByStatus(status: Status): Promise<T[]>;
}
```

---

## 3. Domain Error Handling

```typescript
// errors.ts — Typed domain errors.

export enum ErrorCode {
  NotFound = 'NOT_FOUND',
  InvalidState = 'INVALID_STATE',
  Validation = 'VALIDATION',
  Duplicate = 'DUPLICATE',
  InvalidTransition = 'INVALID_TRANSITION',
  NotFound = 'NOT_FOUND',
}

export class DomainError extends Error {
  constructor(
    public readonly code: ErrorCode,
    message: string,
    public readonly cause?: Error,
  ) {
    super(message);
    this.name = 'DomainError';
  }

  static notFound(id: string): DomainError {
    return new DomainError(ErrorCode.NotFound, `Resource ${id} not found`);
  }
}

export class NotFoundError extends DomainError {
  constructor(id: string) {
    super(ErrorCode.NotFound, `Resource ${id} not found`);
    this.name = 'NotFoundError';
  }
}
```

### Rules
- ✅ Use typed error classes with `ErrorCode` enum
- ✅ Extend `Error` with proper `this.name = 'XxxError'`
- ✅ Create subclasses for common errors (`NotFoundError`, `ValidationError`)
- ❌ Never use bare strings as errors
- ❌ Never return `Error` from domain — throw typed errors

---

## 4. Application Layer — Services

```typescript
// service.ts — Application service.

export interface Service {
  execute(cmd: Command): Promise<Entity>;
  getById(id: string): Promise<Entity>;
}

export class EntityService implements Service {
  constructor(
    private readonly repo: Repository,
    private readonly logger: Logger,
  ) {}

  async execute(cmd: Command): Promise<Entity> {
    const entity = new Entity();
    entity.transition(cmd.desiredStatus);
    
    await this.repo.save(entity);
    this.logger.info('Entity executed', { id: entity.id });
    return entity;
  }

  async getById(id: string): Promise<Entity> {
    const entity = await this.repo.findById(id);
    if (!entity) throw NotFoundError.forId(id);
    return entity;
  }
}
```

---

## 5. Infrastructure Layer

```typescript
// infrastructure/repository/typeorm-repo.ts

export class TypeOrmRepository implements Repository {
  constructor(
    private readonly dataSource: DataSource,
    private readonly logger: Logger,
  ) {}

  async findById(id: string): Promise<Entity | null> {
    const model = await this.dataSource.getRepository(EntityModel)
      .findOneBy({ id });
    return model ? model.toDomain() : null;
  }

  async save(entity: Entity): Promise<void> {
    const model = EntityModel.fromDomain(entity);
    await this.dataSource.getRepository(EntityModel).save(model);
  }

  async delete(id: string): Promise<void> {
    await this.dataSource.getRepository(EntityModel).delete(id);
  }
}
```

---

## 6. Interfaces Layer — HTTP Controllers

```typescript
// interfaces/controller.ts

import { Router, Request, Response } from 'express';

export function createRouter(svc: Service): Router {
  const router = Router();

  router.post('/execute', async (req: Request, res: Response) => {
    try {
      const cmd = ExecuteRequest.parse(req.body); // Zod validation
      const entity = await svc.execute(cmd);
      res.status(201).json(entityToResponse(entity));
    } catch (err) {
      if (err instanceof DomainError) {
        res.status(errorHttpStatus(err.code)).json({ error: err.message });
        return;
      }
      if (err instanceof ZodError) {
        res.status(400).json({ error: 'Validation failed', details: err.errors });
        return;
      }
      console.error('Unexpected error:', err);
      res.status(500).json({ error: 'Internal server error' });
    }
  });

  return router;
}
```

### Rules
- ✅ Controllers translate HTTP to application calls — no business logic
- ✅ Use Zod for request validation
- ✅ Map domain errors to HTTP status codes
- ✅ Use Express/Fastify routers, keep handlers thin

---

## 7. Testing Patterns

### Unit Tests

```typescript
describe('Entity', () => {
  it('transitions from pending to active', () => {
    const entity = new Entity();
    entity.transition('active');
    expect(entity.status).toBe('active');
  });

  it('throws on invalid transition', () => {
    const entity = new Entity();
    entity.transition('active');
    expect(() => entity.transition('pending')).toThrow(DomainError);
  });
});
```

### Integration Tests

```typescript
describe('TypeOrmRepository', () => {
  let dataSource: DataSource;
  
  beforeAll(async () => {
    dataSource = await setupTestDatabase();
  });

  it('persists and retrieves entity', async () => {
    const repo = new TypeOrmRepository(dataSource, pino());
    const entity = new Entity();
    
    await repo.save(entity);
    const found = await repo.findById(entity.id);
    
    expect(found).not.toBeNull();
    expect(found!.id).toBe(entity.id);
  });
});
```

---

## 8. Anti-Patterns — NEVER DO

```typescript
// ❌ any types
function process(data: any)  // BAD — use proper types

// ❌ Business logic in controllers
router.post('/', (req, res) => {
  // domain logic here  // BAD
})

// ❌ Direct DB access from controllers
const user = await db.users.find()  // BAD — use repository

// ❌ Mutable value objects
class Money {
  amount: number  // BAD — should be readonly
}

// ❌ Throwing non-Error types
throw 'something went wrong'  // BAD

// ❌ Ignoring promise rejections
processAsync()  // BAD — missing await
```

---

## 9. Project-Level Structure

```
src/
  module/                    // One directory per bounded context
    domain/
      entity.ts
      value.ts
      repository.ts
      errors.ts
    application/
      service.ts
      dto.ts
    infrastructure/
      repository/
    interfaces/
      controller.ts
      dto.ts
  shared/
    logger.ts
    database.ts
  index.ts
package.json
tsconfig.json
```

---

*Version: 1.0.0*
*Last updated: 2026-07-03*
*Source: Guardian DDD patterns + TypeScript best practices + context7 (/microsoft/typescript)*

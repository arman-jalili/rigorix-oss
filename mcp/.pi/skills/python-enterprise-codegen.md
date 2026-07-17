---
name: python-enterprise-codegen
description: Full reference for Python enterprise code generation with DDD + Clean Architecture. Covers module structure, dataclass entities, FastAPI controllers, error handling, SQLAlchemy repositories, testing, and anti-patterns. Loaded on-demand via agents/python-codegen.md skill.
---

# Python Enterprise Code Generation — DDD + Clean Architecture

> Canonical skill for generating production-grade Python code following Clean Architecture with DDD.
> All code MUST follow these patterns. Validators enforce compliance.
>
> Source: Clean Architecture principles + Python best practices + DDD patterns from context7.

---

## 1. Project Structure — Clean Architecture with DDD

Every bounded context follows the same 4-layer structure:

```
module/
├── domain/                     # Pure domain entities, value objects, events
│   ├── __init__.py
│   ├── entity.py               # Aggregate roots and entities
│   ├── value.py                # Value objects (dataclasses)
│   ├── event.py                # Domain event payloads
│   ├── repository.py           # Repository interfaces (ABC)
│   └── errors.py               # Typed exceptions
├── application/                # Service interfaces and DTOs
│   ├── __init__.py
│   ├── service.py              # Service interface definitions
│   └── dto.py                  # Input/Output DTOs (pydantic)
├── infrastructure/             # Repository implementations, external adapters
│   ├── __init__.py
│   ├── repository/             # Repository implementations
│   │   ├── sqlalchemy_repo.py
│   │   └── in_memory_repo.py
│   └── database.py             # DB connection, migrations
└── interfaces/                 # API contracts (HTTP)
    ├── __init__.py
    ├── controller.py           # FastAPI route handlers
    ├── middleware.py            # Auth, validation middleware
    └── dto.py                  # Request/Response schemas (pydantic)
```

### Dependency Direction Rule (Inward Dependency)

```
domain → application → infrastructure → interfaces
         ↑                    ↑
         └── inner layers never depend on outer layers
```

- **domain/** — depends on nothing except stdlib
- **application/** — depends on domain
- **infrastructure/** — depends on domain + application
- **interfaces/** — depends on application

---

## 2. Domain Layer — Entities & Value Objects

### Entity

```python
# domain/entity.py

from __future__ import annotations
from dataclasses import dataclass, field
from datetime import datetime, timezone
from uuid import uuid4, UUID
from enum import Enum
from .errors import DomainError, ErrorCode


class Status(Enum):
    PENDING = "pending"
    ACTIVE = "active"
    COMPLETED = "completed"
    FAILED = "failed"

    def can_transition_to(self, target: Status) -> bool:
        transitions = {
            Status.PENDING: {Status.ACTIVE},
            Status.ACTIVE: {Status.COMPLETED, Status.FAILED},
            Status.COMPLETED: set(),
            Status.FAILED: set(),
        }
        return target in transitions[self]


@dataclass
class Entity:
    id: UUID = field(default_factory=uuid4)
    status: Status = Status.PENDING
    created_at: datetime = field(default_factory=lambda: datetime.now(timezone.utc))
    updated_at: datetime = field(default_factory=lambda: datetime.now(timezone.utc))

    def transition(self, new_status: Status) -> None:
        if not self.status.can_transition_to(new_status):
            raise DomainError(
                ErrorCode.INVALID_TRANSITION,
                f"Cannot transition from {self.status.value} to {new_status.value}",
            )
        self.status = new_status
        self.updated_at = datetime.now(timezone.utc)
```

### Value Object

```python
# domain/value.py

from __future__ import annotations
from dataclasses import dataclass
from .errors import DomainError, ErrorCode


@dataclass(frozen=True)
class Money:
    amount: int  # Stored in smallest currency unit (cents)
    currency: str

    def __post_init__(self) -> None:
        if self.amount < 0:
            raise DomainError(ErrorCode.VALIDATION, "Amount must be non-negative")
        if len(self.currency) != 3:
            raise DomainError(ErrorCode.VALIDATION, "Currency must be ISO 4217")

    def __add__(self, other: Money) -> Money:
        if self.currency != other.currency:
            raise DomainError(ErrorCode.VALIDATION, "Currency mismatch")
        return Money(self.amount + other.amount, self.currency)
```

### Repository Interface

```python
# domain/repository.py

from abc import ABC, abstractmethod
from uuid import UUID
from .entity import Entity, Status


class Repository(ABC):
    @abstractmethod
    async def find_by_id(self, id: UUID) -> Entity | None: ...

    @abstractmethod
    async def save(self, entity: Entity) -> None: ...

    @abstractmethod
    async def delete(self, id: UUID) -> None: ...

    @abstractmethod
    async def find_by_status(self, status: Status) -> list[Entity]: ...
```

---

## 3. Domain Error Handling

```python
# domain/errors.py

from enum import Enum


class ErrorCode(Enum):
    NOT_FOUND = "NOT_FOUND"
    INVALID_STATE = "INVALID_STATE"
    VALIDATION = "VALIDATION"
    DUPLICATE = "DUPLICATE"
    INVALID_TRANSITION = "INVALID_TRANSITION"


class DomainError(Exception):
    def __init__(self, code: ErrorCode, message: str, cause: Exception | None = None) -> None:
        self.code = code
        self.cause = cause
        super().__init__(f"[{code.value}] {message}")


class NotFoundError(DomainError):
    def __init__(self, id: str) -> None:
        super().__init__(ErrorCode.NOT_FOUND, f"Resource {id} not found")


class ValidationError(DomainError):
    def __init__(self, message: str) -> None:
        super().__init__(ErrorCode.VALIDATION, message)
```

### Rules
- ✅ Use typed exception classes with `ErrorCode`
- ✅ Inherit from `DomainError` for all domain exceptions
- ✅ Use `@dataclass(frozen=True)` for value objects
- ✅ Use `ABC` for repository interfaces
- ❌ Don't use bare `Exception` for domain errors
- ❌ Don't leak ORM models into domain layer

---

## 4. Application Layer — Services

```python
# application/service.py

from uuid import UUID
from ..domain.entity import Entity, Status
from ..domain.repository import Repository


class Service:
    def __init__(self, repo: Repository, logger: Logger) -> None:
        self._repo = repo
        self._logger = logger

    async def execute(self, cmd: Command) -> Entity:
        entity = Entity()
        entity.transition(cmd.desired_status)
        await self._repo.save(entity)
        self._logger.info("Entity executed", extra={"id": str(entity.id)})
        return entity

    async def get_by_id(self, id: UUID) -> Entity:
        entity = await self._repo.find_by_id(id)
        if entity is None:
            raise NotFoundError(str(id))
        return entity
```

---

## 5. Infrastructure Layer — Repository Implementation

```python
# infrastructure/repository/sqlalchemy_repo.py

from uuid import UUID
from sqlalchemy.ext.asyncio import AsyncSession
from ...domain.entity import Entity, Status
from ...domain.repository import Repository


class SQLAlchemyRepository(Repository):
    def __init__(self, session: AsyncSession) -> None:
        self._session = session

    async def find_by_id(self, id: UUID) -> Entity | None:
        model = await self._session.get(EntityModel, id)
        return model.to_domain() if model else None

    async def save(self, entity: Entity) -> None:
        model = EntityModel.from_domain(entity)
        self._session.add(model)
        await self._session.flush()
```

---

## 6. Interfaces Layer — FastAPI Controllers

```python
# interfaces/controller.py

from fastapi import APIRouter, HTTPException, status
from pydantic import BaseModel
from ..application.service import Service
from ..domain.errors import DomainError, ErrorCode

router = APIRouter()

def create_controller(svc: Service) -> APIRouter:
    @router.post("/execute", status_code=status.HTTP_201_CREATED)
    async def execute(cmd: ExecuteRequest) -> EntityResponse:
        try:
            entity = await svc.execute(cmd.to_domain())
            return EntityResponse.from_domain(entity)
        except DomainError as e:
            raise HTTPException(
                status_code=_error_http_status(e.code),
                detail=e.message,
            )

    return router


def _error_http_status(code: ErrorCode) -> int:
    mapping = {
        ErrorCode.NOT_FOUND: 404,
        ErrorCode.VALIDATION: 400,
        ErrorCode.DUPLICATE: 409,
        ErrorCode.INVALID_STATE: 409,
    }
    return mapping.get(code, 500)
```

---

## 7. Testing Patterns

```python
# tests/test_entity.py
import pytest
from uuid import uuid4
from module.domain.entity import Entity, Status
from module.domain.errors import DomainError


class TestEntity:
    def test_transitions_pending_to_active(self) -> None:
        entity = Entity()
        entity.transition(Status.ACTIVE)
        assert entity.status == Status.ACTIVE

    def test_invalid_transition_raises(self) -> None:
        entity = Entity()
        entity.transition(Status.ACTIVE)
        with pytest.raises(DomainError, match="INVALID_TRANSITION"):
            entity.transition(Status.PENDING)
```

---

## 8. Anti-Patterns — NEVER DO

```python
# ❌ Business logic in __init__.py
# ❌ Global state at module level
db = Session()  # BAD

# ❌ ORM models in domain
from sqlalchemy import Column  # BAD — domain is ORM-free

# ❌ Mutable default arguments
def process(items=[]):  # BAD — use None

# ❌ Broad exception handling
try:
    ...
except Exception:  # BAD — catch specific exceptions
    pass

# ❌ Business logic in FastAPI routes
@router.post("/")
async def handler(payload: dict):  # BAD — use pydantic models + service layer

# ❌ Controller/route with abstract interface (routes are concrete only)
class AbstractUserController(ABC): ...  # BAD — FastAPI routes ARE the contract

# ❌ Route importing repository directly (bypasses service layer)
@router.get("/")
async def handler(repo=Depends(get_repo)):  # BAD — use service, not repository
    data = await repo.find_all()

# ❌ Repository interface/ABC in infrastructure layer
class AbstractUserRepository(ABC): ...  # BAD — interfaces belong in domain/

# ✅ Correct: repository ABC in domain/, impl in infrastructure/
# domain/repository.py
class AbstractUserRepository(ABC): ...  # ✅
# infrastructure/repository.py
class PostgresUserRepository(AbstractUserRepository): ...
```

---

## 9. Project-Level Structure

```
src/
  module/                    # One directory per bounded context
    domain/
      __init__.py
      entity.py
      value.py
      repository.py
      errors.py
    application/
      __init__.py
      service.py
      dto.py
    infrastructure/
      __init__.py
      repository/
    interfaces/
      __init__.py
      controller.py
      dto.py
  shared/
    logger.py
    database.py
  main.py
tests/
pyproject.toml
```

---

*Version: 1.0.0*
*Last updated: 2026-07-03*
*Source: Guardian DDD patterns + Python best practices + context7 (/fastapi/fastapi)*

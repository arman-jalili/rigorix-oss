---
name: go-enterprise-codegen
description: Full reference for Go enterprise code generation with DDD + Clean Architecture. Covers module structure, domain entities, error handling, service layer, repository pattern, HTTP handlers, testing, and anti-patterns. Loaded on-demand via agents/go-codegen.md skill.
---

# Go Enterprise Code Generation — DDD + Clean Architecture

> Canonical skill for generating production-grade Go code following Clean Architecture with DDD.
> All code MUST follow these patterns. Validators enforce compliance.
>
> Source: Clean Architecture principles + Go standard library patterns + DDD patterns from context7.

---

## 1. Project Structure — Clean Architecture with DDD

Every bounded context follows the same 4-layer structure:

```
module/
├── domain/                # Pure domain entities, value objects, events
│   ├── entity.go          # Aggregate roots and entities
│   ├── value.go           # Value objects
│   ├── event.go           # Domain event payloads
│   ├── error.go           # Typed error definitions
│   └── repository.go      # Repository interfaces
├── application/           # Service interfaces and DTOs
│   ├── service.go         # Service interface definitions
│   └── dto.go             # Input/Output DTOs
├── infrastructure/        # Repository implementations, external adapters
│   ├── repository/        # Repository implementations (GORM, SQL)
│   │   ├── module_repo.go
│   │   └── module_test.go
│   └── persistence/       # Database connection, migrations
└── interfaces/            # API contracts (HTTP)
    ├── handler.go         # HTTP handler definitions
    ├── router.go          # Route registration
    └── dto.go             # Request/Response DTOs
```

### Dependency Direction Rule (Inward Dependency)

```
domain → application → infrastructure → interfaces
         ↑                    ↑
         └── inner layers never depend on outer layers
```

- **domain/** — depends on nothing except standard library
- **application/** — depends on domain
- **infrastructure/** — depends on domain + application
- **interfaces/** — depends on application (HTTP/event handlers translate to domain calls)

### Package Naming Convention

```go
package module       // domain/ — single domain package
package application  // application/
package repository   // infrastructure/repository/
package handler      // interfaces/
```

---

## 2. Domain Layer Entities

### Entity with Encapsulated State

```go
// entity.go — Domain entity with encapsulate state and behavior.

// Entity is a domain object with identity (ID field).
type Entity struct {
    id        primitive.ObjectID
    status    Status
    createdAt time.Time
    updatedAt time.Time
}

func NewEntity(id primitive.ObjectID) *Entity {
    return &Entity{
        id:        id,
        status:    StatusPending,
        createdAt: time.Now(),
        updatedAt: time.Now(),
    }
}

// ID returns the immutable entity identifier.
func (e *Entity) ID() primitive.ObjectID { return e.id }

// Status returns the current lifecycle status.
func (e *Entity) Status() Status { return e.status }

// transition encapsulates state mutations — never expose setters.
func (e *Entity) transition(newStatus Status) error {
    if !e.status.CanTransitionTo(newStatus) {
        return &DomainError{
            Code:    ErrInvalidTransition,
            Message: fmt.Sprintf("cannot transition from %s to %s", e.status, newStatus),
        }
    }
    e.status = newStatus
    e.updatedAt = time.Now()
    return nil
}
```

### Value Object

```go
// value.go — Immutable value object.

// Money represents an amount in the smallest currency unit.
type Money struct {
    amount   int64
    currency string
}

func NewMoney(amount int64, currency string) (Money, error) {
    if amount < 0 {
        return Money{}, &DomainError{Code: ErrValidation, Message: "amount must be non-negative"}
    }
    if len(currency) != 3 {
        return Money{}, &DomainError{Code: ErrValidation, Message: "currency must be ISO 4217"}
    }
    return Money{amount: amount, currency: strings.ToUpper(currency)}, nil
}

func (m Money) Amount() int64           { return m.amount }
func (m Money) Currency() string        { return m.currency }
func (m Money) Add(other Money) (Money, error) {
    if m.currency != other.currency {
        return Money{}, &DomainError{Code: ErrCurrencyMismatch, Message: "currency mismatch"}
    }
    return Money{amount: m.amount + other.amount, currency: m.currency}, nil
}
```

### Repository Interface

```go
// repository.go — Repository interface defined in domain, implemented in infrastructure.

type Repository interface {
    FindByID(ctx context.Context, id primitive.ObjectID) (*Entity, error)
    Save(ctx context.Context, entity *Entity) error
    Delete(ctx context.Context, id primitive.ObjectID) error
    FindByStatus(ctx context.Context, status Status) ([]*Entity, error)
}
```

---

## 3. Domain Error Handling

```go
// error.go — Typed domain errors.

type ErrorCode string

const (
    ErrNotFound         ErrorCode = "NOT_FOUND"
    ErrInvalidState     ErrorCode = "INVALID_STATE"
    ErrValidation       ErrorCode = "VALIDATION"
    ErrDuplicate        ErrorCode = "DUPLICATE"
    ErrCurrencyMismatch ErrorCode = "CURRENCY_MISMATCH"
    ErrInvalidTransition ErrorCode = "INVALID_TRANSITION"
)

type DomainError struct {
    Code    ErrorCode
    Message string
    Err     error
}

func (e *DomainError) Error() string {
    if e.Err != nil {
        return fmt.Sprintf("[%s] %s: %v", e.Code, e.Message, e.Err)
    }
    return fmt.Sprintf("[%s] %s", e.Code, e.Message)
}

func (e *DomainError) Unwrap() error { return e.Err }

// Sentinel errors for direct comparison.
var (
    ErrNotFoundInstance      = &DomainError{Code: ErrNotFound, Message: "resource not found"}
    ErrDuplicateInstance     = &DomainError{Code: ErrDuplicate, Message: "duplicate resource"}
)
```

### Rules
- ✅ Use `*DomainError` with typed `ErrorCode` for all domain errors
- ✅ Sentinel errors for direct comparison via `errors.Is()`
- ✅ Use `fmt.Errorf("context: %w", err)` for wrapping with context
- ✅ Use `errors.As()` to extract typed errors from wrapped chains
- ❌ Never use `panic` for error handling
- ❌ Never use bare strings as errors
- ❌ Never use `log.Fatal` in library code

---

## 4. Application Layer — Services

```go
// service.go — Application service interface.

type Service interface {
    Execute(ctx context.Context, cmd Command) (*Entity, error)
    GetByID(ctx context.Context, id primitive.ObjectID) (*Entity, error)
}

type service struct {
    repo   Repository
    logger *slog.Logger
}

func NewService(repo Repository, logger *slog.Logger) Service {
    return &service{repo: repo, logger: logger}
}

func (s *service) Execute(ctx context.Context, cmd Command) (*Entity, error) {
    entity := NewEntity(primitive.NewObjectID())
    // Domain logic — validation, state transitions
    if err := entity.transition(cmd.DesiredStatus); err != nil {
        return nil, fmt.Errorf("execute: %w", err)
    }
    if err := s.repo.Save(ctx, entity); err != nil {
        return nil, fmt.Errorf("save entity: %w", err)
    }
    s.logger.InfoContext(ctx, "entity executed", "id", entity.ID())
    return entity, nil
}
```

### Rules
- ✅ Services orchestrate domain operations — no business logic
- ✅ Services depend on domain interfaces (Repository), not concrete implementations
- ✅ Use `slog.Logger` for structured logging
- ✅ Each service method is a single use case

---

## 5. Infrastructure Layer — Repository Implementation

```go
// repository/module_repo.go — GORM-based repository implementation.

type gormRepository struct {
    db     *gorm.DB
    logger *slog.Logger
}

func NewGormRepository(db *gorm.DB, logger *slog.Logger) Repository {
    return &gormRepository{db: db, logger: logger}
}

func (r *gormRepository) FindByID(ctx context.Context, id primitive.ObjectID) (*Entity, error) {
    var model entityModel
    if err := r.db.WithContext(ctx).First(&model, "id = ?", id).Error; err != nil {
        if errors.Is(err, gorm.ErrRecordNotFound) {
            return nil, fmt.Errorf("find by id %s: %w", id.Hex(), ErrNotFoundInstance)
        }
        return nil, fmt.Errorf("find by id %s: %w", id.Hex(), err)
    }
    return model.toEntity(), nil
}
```

### Rules
- ✅ Repository implementations live in `infrastructure/repository/`
- ✅ Each repository implements interfaces defined in `domain/`
- ✅ Use GORM, sqlx, or standard `database/sql` — never leak DB types into domain
- ✅ Convert ORM models to domain entities inside the repository

---

## 6. Interfaces Layer — HTTP Handlers

```go
// handler.go — HTTP handler that translates HTTP to application calls.

type Handler struct {
    svc    application.Service
    logger *slog.Logger
}

func NewHandler(svc application.Service, logger *slog.Logger) *Handler {
    return &Handler{svc: svc, logger: logger}
}

func (h *Handler) Execute(w http.ResponseWriter, r *http.Request) {
    var req ExecuteRequest
    if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
        http.Error(w, `{"error":"invalid request"}`, http.StatusBadRequest)
        return
    }

    entity, err := h.svc.Execute(r.Context(), req.ToCommand())
    if err != nil {
        var de *DomainError
        if errors.As(err, &de) {
            http.Error(w, fmt.Sprintf(`{"error":"%s"}`, de.Message), errorHTTPStatus(de.Code))
            return
        }
        h.logger.ErrorContext(r.Context(), "execute failed", "error", err)
        http.Error(w, `{"error":"internal error"}`, http.StatusInternalServerError)
        return
    }

    w.Header().Set("Content-Type", "application/json")
    json.NewEncoder(w).Encode(entityToResponse(entity))
}
```

### Rules
- ✅ Handlers translate HTTP to application calls — no business logic
- ✅ Always use structured error responses (JSON)
- ✅ Map domain errors to appropriate HTTP status codes
- ✅ Use standard `net/http` or chi/gin router, not framework-specific types in handlers

---

## 7. Configuration — Environment-Based

```go
type Config struct {
    Port        int           `envconfig:"PORT" default:"8080"`
    DatabaseURL string        `envconfig:"DATABASE_URL" required:"true"`
    LogLevel    slog.Level    `envconfig:"LOG_LEVEL" default:"info"`
    APIKey      string        `envconfig:"API_KEY"`  // Loaded from env, never from config file
}

func LoadConfig() (*Config, error) {
    var cfg Config
    if err := envconfig.Process("APP", &cfg); err != nil {
        return nil, fmt.Errorf("load config: %w", err)
    }
    return &cfg, nil
}
```

### Rules
- ✅ Use `envconfig` or `viper` for configuration
- ✅ CLI flags > Environment > Config file > Defaults
- ✅ Secrets loaded ONLY from environment variables
- ✅ Validate on startup — fail fast

---

## 8. Testing Patterns

### Unit Tests

```go
func TestEntity_Transition(t *testing.T) {
    e := NewEntity(primitive.NewObjectID())
    
    // Initial state
    assert.Equal(t, StatusPending, e.Status())
    
    // Valid transition
    err := e.transition(StatusActive)
    assert.NoError(t, err)
    assert.Equal(t, StatusActive, e.Status())
    
    // Invalid transition
    err = e.transition(StatusPending) // Can't go back
    assert.Error(t, err)
    var de *DomainError
    assert.True(t, errors.As(err, &de))
    assert.Equal(t, ErrInvalidTransition, de.Code)
}
```

### Repository Tests with Testcontainers

```go
func TestGormRepository(t *testing.T) {
    // Spin up test PostgreSQL container
    testDB, err := setupTestDatabase(t)
    require.NoError(t, err)
    
    repo := NewGormRepository(testDB, slog.Default())
    
    entity := NewEntity(primitive.NewObjectID())
    err = repo.Save(context.Background(), entity)
    require.NoError(t, err)
    
    found, err := repo.FindByID(context.Background(), entity.ID())
    require.NoError(t, err)
    assert.Equal(t, entity.Status(), found.Status())
}
```

---

## 9. Anti-Patterns — NEVER DO

```go
// ❌ Panic for error handling
if err != nil { panic(err) }  // BAD

// ❌ Stringly-typed errors
errors.New("something went wrong")  // BAD — use typed DomainError

// ❌ Global state
var db *gorm.DB  // BAD — use dependency injection

// ❌ Business logic in handlers
func Handler(w http.ResponseWriter, r *http.Request) {
    // complex business logic here  // BAD — belongs in domain/application
}

// ❌ Leaking DB models to domain
type User struct { gorm.Model }  // BAD — domain should not know about GORM

// ❌ Ignoring errors
json.NewEncoder(w).Encode(data)  // BAD — always handle errors

// ❌ Background goroutines in handlers
go doSomething()  // BAD — use proper worker pools with lifecycle management

// ❌ Controller/handler with interface (handlers are concrete only)
type UserHandler interface { ... }  // BAD — http.HandlerFunc IS the contract
type userHandlerImpl struct { ... }  // BAD — pointless indirection

// ❌ Handler importing repository directly (bypasses service layer)
func Handler(w http.ResponseWriter, r *http.Request) {
    repo.FindAll()  // BAD — use application service, not repository
}

// ❌ Repository interface in infrastructure layer
// BAD: interfaces belong in domain/
type UserRepository interface { ... }  // put this in domain/repository.go

// ✅ Correct: repository interface in domain/
type UserRepository interface { ... }  // ✅ in domain/repository.go
// Implementation in infrastructure/repository/
type postgresUserRepository struct { ... }
```

---

## 10. Go Project-Level Structure

```
cmd/
  server/
    main.go                    // Entry point, DI setup
internal/
  module/                      // One directory per bounded context
    domain/
    application/
    infrastructure/
      repository/
    interfaces/
pkg/
  framework/                    // Shared infrastructure
    database.go
    logger.go
    router.go
docker-compose.yml
Dockerfile
go.mod
```

---

*Version: 1.0.0*
*Last updated: 2026-07-03*
*Source: Guardian DDD patterns + Clean Architecture + context7 (/wesionaryteam/go_clean_architecture, /golang/go)*

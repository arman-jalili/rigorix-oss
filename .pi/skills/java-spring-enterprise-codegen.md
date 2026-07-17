---
name: java-spring-enterprise-codegen
description: Full reference for Java/Spring Boot enterprise code generation with DDD + Clean Architecture. Covers module structure, JPA entities, REST controllers, service layer, exception handling, MockMvc testing, and anti-patterns. Loaded on-demand via agents/java-codegen.md skill.
---

# Java/Spring Boot Enterprise Code Generation — DDD + Clean Architecture

> Canonical skill for generating production-grade Java code with Spring Boot following Clean Architecture with DDD.
> All code MUST follow these patterns. Validators enforce compliance.
>
> Source: Clean Architecture principles + Spring Boot best practices + DDD patterns from context7 (/jkazama/ddd-java, /ardalis/cleanarchitecture, /websites/spring_io_spring-boot).

---

## 1. Project Structure — Clean Architecture with DDD

Every bounded context follows the same 4-layer structure:

```
module/
├── domain/                     # Pure domain entities, value objects, events
│   ├── model/                  # Aggregate roots and entities
│   │   ├── Entity.java
│   │   └── Status.java
│   ├── vo/                     # Value objects
│   │   └── Money.java
│   ├── event/                  # Domain event payloads
│   │   └── EntityEvent.java
│   ├── repository/             # Repository interfaces
│   │   └── EntityRepository.java
│   └── error/                  # Typed exception classes
│       ├── DomainException.java   # Abstract base for all domain errors
│       ├── DomainError.java      # Concrete domain error
│       └── ErrorCode.java        # Machine-readable error codes
├── application/                # Service interfaces and DTOs
│   ├── service/                # Service interfaces
│   │   └── EntityService.java
│   ├── dto/                    # Input/Output DTOs
│   │   ├── Command.java
│   │   └── EntityResponse.java
│   └── impl/                   # Service implementations
│       └── EntityServiceImpl.java
├── infrastructure/             # Repository implementations, external adapters
│   ├── repository/             # JPA repository implementations
│   │   └── JpaEntityRepository.java
│   └── persistence/            # JPA entities, migrations
└── interfaces/                 # API contracts (HTTP)
    ├── rest/                   # REST controllers
    │   └── EntityController.java
    ├── dto/                    # Request/Response DTOs
    └── advice/                 # Exception handlers
        └── GlobalExceptionHandler.java
```

### Dependency Direction Rule

```
domain → application → infrastructure → interfaces
```

- **domain/** — pure Java, no Spring annotations
- **application/** — depends on domain, uses Spring `@Service`
- **infrastructure/** — depends on domain + application, uses Spring Data JPA
- **interfaces/** — depends on application, uses Spring `@RestController`

---

## 2. Domain Layer

### Entity

```java
// domain/model/Entity.java

package com.project.module.domain.model;

import com.project.module.domain.vo.Money;
import com.project.module.domain.error.DomainError;
import java.time.Instant;
import java.util.UUID;

public class Entity {
    private final UUID id;
    private Status status;
    private final Instant createdAt;
    private Instant updatedAt;

    public Entity() {
        this.id = UUID.randomUUID();
        this.status = Status.PENDING;
        this.createdAt = Instant.now();
        this.updatedAt = Instant.now();
    }

    public UUID getId() { return id; }
    public Status getStatus() { return status; }
    public Instant getCreatedAt() { return createdAt; }

    public void transition(Status newStatus) {
        if (!status.canTransitionTo(newStatus)) {
            throw DomainError.invalidTransition(status, newStatus);
        }
        this.status = newStatus;
        this.updatedAt = Instant.now();
    }
}
```

### Value Object

```java
// domain/vo/Money.java

package com.project.module.domain.vo;

import com.project.module.domain.error.DomainError;

public record Money(long amount, String currency) {

    public Money {
        if (amount < 0) throw new DomainError("Amount must be non-negative");
        if (currency == null || currency.length() != 3)
            throw new DomainError("Currency must be ISO 4217");
    }

    public Money add(Money other) {
        if (!this.currency.equals(other.currency)) {
            throw new DomainError("Currency mismatch");
        }
        return new Money(this.amount + other.amount, this.currency);
    }
}
```

### Repository Interface

```java
// domain/repository/EntityRepository.java

package com.project.module.domain.repository;

import com.project.module.domain.model.Entity;
import com.project.module.domain.model.Status;
import java.util.List;
import java.util.Optional;
import java.util.UUID;

public interface EntityRepository {
    Optional<Entity> findById(UUID id);
    void save(Entity entity);
    void delete(UUID id);
    List<Entity> findByStatus(Status status);
}
```

### Status Enum with Transitions

```java
// domain/model/Status.java

public enum Status {
    PENDING,
    ACTIVE,
    COMPLETED,
    FAILED;

    public boolean canTransitionTo(Status target) {
        return switch (this) {
            case PENDING -> target == ACTIVE;
            case ACTIVE -> target == COMPLETED || target == FAILED;
            case COMPLETED, FAILED -> false;
        };
    }
}
```

---

## 3. Domain Error Handling

```java
// domain/error/DomainException.java

package com.project.module.domain.error;

/**
 * Base exception for all domain errors.
 * Extending this (rather than RuntimeException directly) allows
 * catching all domain-level errors in a single handler.
 */
public abstract class DomainException extends RuntimeException {
    private final ErrorCode code;

    protected DomainException(ErrorCode code, String message) {
        super(message);
        this.code = code;
    }

    public ErrorCode getCode() { return code; }
}

// domain/error/DomainError.java

package com.project.module.domain.error;

public class DomainError extends DomainException {

    public DomainError(String message) {
        super(ErrorCode.VALIDATION, message);
    }

    public DomainError(ErrorCode code, String message) {
        super(code, message);
    }

    public static DomainError notFound(String id) {
        return new DomainError(ErrorCode.NOT_FOUND, "Resource " + id + " not found");
    }

    public static DomainError invalidTransition(Status current, Status target) {
        return new DomainError(
            ErrorCode.INVALID_TRANSITION,
            "Cannot transition from " + current + " to " + target
        );
    }
}

// domain/error/ErrorCode.java

public enum ErrorCode {
    NOT_FOUND,
    INVALID_STATE,
    VALIDATION,
    DUPLICATE,
    INVALID_TRANSITION
}
```

### Rules
- ✅ Use `DomainException` subclasses for domain errors (never `RuntimeException` directly)
- ✅ Define `DomainException` as an abstract base in `domain/error/`
- ✅ Use `ErrorCode` enum for machine-readable error types
- ✅ Use `record` for value objects (Java 16+)
- ❌ Don't use Spring annotations in domain layer
- ❌ Don't use checked exceptions for domain errors

---

## 4. Application Layer

```java
// application/service/EntityService.java

package com.project.module.application.service;

import com.project.module.domain.model.Entity;
import com.project.module.application.dto.Command;
import java.util.UUID;

public interface EntityService {
    Entity execute(Command cmd);
    Entity getById(UUID id);
}
```

```java
// application/impl/EntityServiceImpl.java

package com.project.module.application.impl;

import com.project.module.domain.model.Entity;
import com.project.module.domain.repository.EntityRepository;
import com.project.module.domain.error.DomainError;
import com.project.module.application.service.EntityService;
import com.project.module.application.dto.Command;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

@Slf4j
@Service
@RequiredArgsConstructor
public class EntityServiceImpl implements EntityService {

    private final EntityRepository repository;

    @Override
    @Transactional
    public Entity execute(Command cmd) {
        var entity = new Entity();
        entity.transition(cmd.desiredStatus());
        repository.save(entity);
        log.info("Entity executed: {}", entity.getId());
        return entity;
    }

    @Override
    @Transactional(readOnly = true)
    public Entity getById(UUID id) {
        return repository.findById(id)
            .orElseThrow(() -> DomainError.notFound(id.toString()));
    }
}
```

---

## 5. Infrastructure Layer — JPA Repository

```java
// infrastructure/repository/JpaEntityRepository.java

package com.project.module.infrastructure.repository;

import com.project.module.domain.model.Entity;
import com.project.module.domain.model.Status;
import com.project.module.domain.repository.EntityRepository;
import jakarta.persistence.EntityManager;
import lombok.RequiredArgsConstructor;
import org.springframework.stereotype.Repository;
import java.util.*;

@Repository
@RequiredArgsConstructor
public class JpaEntityRepository implements EntityRepository {

    private final EntityManager em;

    @Override
    public Optional<Entity> findById(UUID id) {
        return Optional.ofNullable(em.find(Entity.class, id));
    }

    @Override
    public void save(Entity entity) {
        em.persist(entity);
    }

    @Override
    public void delete(UUID id) {
        findById(id).ifPresent(em::remove);
    }

    @Override
    public List<Entity> findByStatus(Status status) {
        return em.createQuery("SELECT e FROM Entity e WHERE e.status = :status", Entity.class)
            .setParameter("status", status)
            .getResultList();
    }
}
```

---

## 6. Interfaces Layer — REST Controllers

```java
// interfaces/rest/EntityController.java

package com.project.module.interfaces.rest;

import com.project.module.application.service.EntityService;
import com.project.module.application.dto.Command;
import com.project.module.interfaces.dto.CreateEntityRequest;
import com.project.module.interfaces.dto.EntityResponse;
import jakarta.validation.Valid;
import lombok.RequiredArgsConstructor;
import org.springframework.http.HttpStatus;
import org.springframework.web.bind.annotation.*;

@RestController
@RequestMapping("/api/v1/entities")
@RequiredArgsConstructor
public class EntityController {

    private final EntityService service;

    @PostMapping
    @ResponseStatus(HttpStatus.CREATED)
    public EntityResponse execute(@Valid @RequestBody CreateEntityRequest request) {
        var entity = service.execute(request.toCommand());
        return EntityResponse.from(entity);
    }

    @GetMapping("/{id}")
    public EntityResponse getById(@PathVariable UUID id) {
        var entity = service.getById(id);
        return EntityResponse.from(entity);
    }
}
```

### Global Exception Handler

```java
// interfaces/advice/GlobalExceptionHandler.java

@RestControllerAdvice
public class GlobalExceptionHandler {

    @ExceptionHandler(DomainError.class)
    @ResponseStatus(HttpStatus.BAD_REQUEST)
    public ErrorResponse handleDomainError(DomainError e) {
        return new ErrorResponse(e.getCode().name(), e.getMessage());
    }

    @ExceptionHandler(NotFoundException.class)
    @ResponseStatus(HttpStatus.NOT_FOUND)
    public ErrorResponse handleNotFound(NotFoundException e) {
        return new ErrorResponse("NOT_FOUND", e.getMessage());
    }

    @ExceptionHandler(MethodArgumentNotValidException.class)
    @ResponseStatus(HttpStatus.BAD_REQUEST)
    public ErrorResponse handleValidation(MethodArgumentNotValidException e) {
        return new ErrorResponse("VALIDATION", "Invalid request body");
    }
}
```

---

## 7. Testing Patterns

```java
@SpringBootTest
@AutoConfigureMockMvc
class EntityControllerTest {

    @Autowired
    private MockMvc mockMvc;

    @MockBean
    private EntityService service;

    @Test
    void shouldCreateEntity() throws Exception {
        var entity = new Entity();
        when(service.execute(any())).thenReturn(entity);

        mockMvc.perform(post("/api/v1/entities")
                .contentType(MediaType.APPLICATION_JSON)
                .content("""
                    {"desiredStatus": "ACTIVE"}
                    """))
            .andExpect(status().isCreated())
            .andExpect(jsonPath("$.id").value(entity.getId().toString()));
    }
}
```

---

## 8. Anti-Patterns — NEVER DO

```java
// ❌ Domain entities with Spring annotations
@Entity
@Table(name = "entities")
public class Entity { ... }  // BAD — domain is JPA-free

// ❌ Business logic in controllers
@PostMapping
public void execute(@RequestBody Request req) {
    // business logic here  // BAD — use service layer
}

// ❌ Field injection
@Autowired
private EntityService service;  // BAD — use constructor injection

// ❌ Mutable value objects without defensive copies
public class Money {
    private int amount;  // BAD — should be final
}

// ❌ Leaking JPA entities to interfaces
@Entity
public class User {
    // used directly in REST responses  // BAD — use DTOs
}

// ❌ Controller with interface (controllers are concrete only)
public interface UserController { ... }  // BAD — @RestController is the contract
public class UserControllerImpl implements UserController { ... }  // BAD — pointless indirection

// ❌ Controller importing repository directly (bypasses service layer)
@RestController
public class UserController {
    private final UserRepository repo;  // BAD — use application service, not repository
}

// ❌ Repository interface in infrastructure layer
package com.project.module.infrastructure.repository;  // BAD — interfaces belong in domain/repository/
public interface UserRepository { ... }

// ✅ Correct: repository interface in domain, impl in infrastructure
package com.project.module.domain.repository;  // ✅ interface in domain
public interface UserRepository { ... }

package com.project.module.infrastructure.repository;  // ✅ impl in infrastructure
@Repository
public class JpaUserRepository implements UserRepository { ... }
```

---

## 9. Project-Level Structure

```
src/main/java/com/company/project/
  module/
    domain/
      model/
      vo/
      event/
      repository/
      error/
    application/
      service/
      dto/
      impl/
    infrastructure/
      repository/
      persistence/
    interfaces/
      rest/
      dto/
      advice/
  shared/
    config/
    logging/
src/test/java/com/company/project/
pom.xml (or build.gradle)
```

---

*Version: 1.0.0*
*Last updated: 2026-07-03*
*Source: Guardian DDD patterns + Spring Boot best practices + context7 (/jkazama/ddd-java, /websites/spring_io_spring-boot)*

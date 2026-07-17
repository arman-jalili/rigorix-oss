---
name: angular-enterprise-codegen
description: Full reference for Angular enterprise code generation with DDD + Clean Architecture. Covers module structure, standalone components, Signal-based state, RxJS patterns, design system integration, lazy loading, testing, and performance. Loaded on-demand via agents/angular-codegen.md skill.
---

# Angular Enterprise Code Generation — DDD + Clean Architecture

> Canonical skill for generating production-grade Angular code with DDD patterns.
> All code MUST follow these patterns. Validators enforce compliance.
>
> For design system and styling patterns, see: `.pi/skills/design-system-enterprise-codegen.md`

---

## 1. Project Structure — DDD with Angular Standalone Components

```
src/
  module/                              # One bounded context
    domain/                            # Pure business logic, zero Angular imports
      entity.ts                        # Domain entities with encapsulated state
      value.ts                         # Immutable value objects
      events.ts                        # Domain events
      errors.ts                        # Typed error classes
    application/                       # Application logic
      store.ts                         # Signal-based state management
      service.ts                       # @Injectable use case service
      facade.ts                        # Facade pattern (service → store)
      dto.ts                           # Command/Query DTOs
    infrastructure/                    # External adapters
      api-client.ts                    # HttpClient wrapper with interceptors
      auth.ts                          # Auth provider SDK wrapper
      analytics.ts                     # Analytics SDK wrapper
    interfaces/                        # UI layer
      page.component.ts                # Route component (standalone)
      list.component.ts                # Smart component
      item.component.ts                # Presentational component
      form.component.ts                # Form with Reactive Forms
  shared/                              # Cross-module shared code
    ui/                                # Design system components
      button/
      card/
      modal/
    lib/                               # Shared utilities
    pipes/                             # Shared Angular pipes
```

### Dependency Rule
```
domain → application → infrastructure → interfaces
                  ↕
            shared/ (no framework deps)
```

- **domain/** — zero Angular imports. Pure TypeScript. No decorators.
- **application/** — imports from domain. Uses `@Injectable` for DI.
- **infrastructure/** — imports from domain + application. Uses Angular `HttpClient`.
- **interfaces/** — imports from application. Standalone components with `@Component`.

---

## 2. Component Architecture

### Smart vs Presentational Components

| Type | Location | Responsibility |
|------|----------|---------------|
| Smart (Page) | `interfaces/page.component.ts` | Route handler, orchestrates data flow |
| Smart (List) | `interfaces/list.component.ts` | Uses store/service, passes data down |
| Presentational | `interfaces/item.component.ts` | Pure @Input/@Output, no DI |
| Form | `interfaces/form.component.ts` | Reactive Forms, validation |

```typescript
// interfaces/list.component.ts — Smart component
@Component({
  selector: 'app-order-list',
  standalone: true,
  imports: [OrderItemComponent, AsyncPipe],
  template: `
    @for (order of orders$ | async; track order.id) {
      <app-order-item [order]="order" (select)="onSelect($event)" />
    } @empty {
      <empty-state />
    }
  `,
})
export class OrderListComponent {
  private readonly service = inject(OrderService);
  readonly orders$ = this.service.orders$;

  onSelect(order: Order): void {
    this.service.selectOrder(order.id);
  }
}

// interfaces/item.component.ts — Presentational component
@Component({
  selector: 'app-order-item',
  standalone: true,
  template: `
    <div (click)="select.emit(order.id)">
      <h3>{{ order.id }}</h3>
      <span [class]="statusClass(order.status)">{{ order.status }}</span>
    </div>
  `,
})
export class OrderItemComponent {
  @Input({ required: true }) order!: Order;
  @Output() select = new EventEmitter<string>();
}
```

---

## 3. State Management with Signals

```typescript
// application/store.ts — Signal-based state
import { signal, computed, Injectable } from '@angular/core';
import { Order, OrderStatus } from '../domain/entity';

@Injectable({ providedIn: 'root' })
export class OrderStore {
  private readonly orders = signal<Order[]>([]);
  private readonly selectedId = signal<string | null>(null);
  private readonly isLoading = signal(false);

  readonly orders$ = this.orders.asReadonly();
  readonly selectedOrder = computed(() => {
    const id = this.selectedId();
    return id ? this.orders().find(o => o.id === id) ?? null : null;
  });
  readonly isLoading$ = this.isLoading.asReadonly();
  readonly pendingCount = computed(() =>
    this.orders().filter(o => o.status === OrderStatus.PENDING).length
  );

  async loadOrders(): Promise<void> {
    this.isLoading.set(true);
    try {
      const orders = await this.api.fetchOrders();
      this.orders.set(orders);
    } finally {
      this.isLoading.set(false);
    }
  }

  selectOrder(id: string): void {
    this.selectedId.set(id);
  }
}
```

### Rules
- ✅ Use `signal()` over `BehaviorSubject` for state (Angular 17+)
- ✅ Use `computed()` for derived state — no manual subscriptions
- ✅ Use `effect()` only for side effects (logging, syncing to localStorage)
- ❌ Don't mix Signals and RxJS in the same store — pick one pattern
- ❌ Don't use `async` pipe with Signals — Signals work with `@for` directly

---

## 4. Data Flow with RxJS

```typescript
// application/service.ts — RxJS for async operations
@Injectable({ providedIn: 'root' })
export class OrderService {
  private readonly http = inject(HttpClient);
  private readonly store = inject(OrderStore);

  // Stream: debounced search → API call → update store
  readonly search = new Subject<string>();

  constructor() {
    this.search.pipe(
      debounceTime(300),
      distinctUntilChanged(),
      switchMap(query => this.http.get<Order[]>(`/api/orders?q=${query}`)),
      catchError(err => {
        console.error('Search failed', err);
        return of([]);
      }),
    ).subscribe(orders => this.store.setOrders(orders));
  }

  // Server mutation with optimistic update
  confirmOrder(id: string): void {
    const previous = this.store.orders$();
    this.store.updateOrder(id, OrderStatus.CONFIRMED);  // Optimistic

    this.http.post(`/api/orders/${id}/confirm`, {}).pipe(
      catchError(err => {
        this.store.setOrders(previous);  // Rollback
        throw err;
      }),
    ).subscribe();
  }
}
```

### Rules
- ✅ Services handle async (HTTP, timers), Stores hold synchronous state
- ✅ Use `switchMap` for search/autocomplete (cancel previous)
- ✅ Use `exhaustMap` for mutations (ignore while in-flight)
- ✅ Always `catchError` in service streams — never let errors propagate unhandled
- ❌ Never subscribe in components — use `async` pipe or `@for` with Signals
- ❌ Never put business logic in RxJS pipes — that's the domain layer's job

---

## 5. Design System & CSS Architecture

### Structure
```
src/
  shared/
    ui/                                  # Design system components
      tokens/                            # Design tokens
        _colors.scss
        _typography.scss
        _spacing.scss
      button/
        button.component.ts
        button.component.scss
        button.test.ts
      card/
      form/
        input/
        select/
```

### Styling Strategy

| Approach | When to use |
|----------|-------------|
| Component SCSS (`@Component styleUrls`) | Component-scoped styles |
| Global SCSS (`styles.scss`) | CSS reset, typography, CSS custom properties |
| CSS Custom Properties | Design tokens, theming |
| Tailwind (via ngClass) | Layout, one-off adjustments |

```scss
// shared/ui/tokens/_colors.scss
:root {
  --color-primary: #0f766e;
  --color-primary-foreground: #ffffff;
  --color-secondary: #64748b;
  --color-destructive: #dc2626;
  --color-background: #ffffff;
  --color-foreground: #0f172a;
}

// button.component.scss
@use '../../tokens/colors' as *;

.button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 6px;
  font-size: 0.875rem;
  font-weight: 500;
  transition: all 0.2s;

  &--primary {
    background: var(--color-primary);
    color: var(--color-primary-foreground);
  }
  &--secondary {
    background: var(--color-secondary);
    color: #fff;
  }
  &--sm { height: 2.25rem; padding: 0 0.75rem; }
  &--md { height: 2.5rem; padding: 0 1rem; }
  &--lg { height: 2.75rem; padding: 0 2rem; }
}
```

```typescript
// button.component.ts
@Component({
  selector: 'app-button',
  standalone: true,
  template: `
    <button [class]="'button button--' + variant() + ' button--' + size()">
      <ng-content />
    </button>
  `,
})
export class ButtonComponent {
  readonly variant = input<'primary' | 'secondary'>('primary');
  readonly size = input<'sm' | 'md' | 'lg'>('md');
}
```

---

## 6. Lazy Loading & Routing

```typescript
// app.routes.ts — Lazy-loaded modules
export const routes: Routes = [
  {
    path: 'orders',
    loadChildren: () => import('./orders/orders.routes'),
    canActivate: [AuthGuard],
  },
  {
    path: 'checkout',
    loadComponent: () => import('./checkout/page.component'),
  },
];

// orders/orders.routes.ts
export default [
  { path: '', component: OrderListComponent },
  { path: ':id', component: OrderDetailComponent },
] satisfies Routes;
```

---

## 7. Error Handling

```typescript
// domain/errors.ts
export class DomainError extends Error {
  constructor(
    public readonly code: string,
    message: string,
    public readonly httpStatus: number = 400,
  ) {
    super(message);
    this.name = 'DomainError';
  }
}

// infrastructure/api-client.ts — HTTP interceptor
@Injectable()
export class ErrorInterceptor implements HttpInterceptor {
  intercept(req: HttpRequest<unknown>, next: HttpHandlerFn): Observable<HttpEvent<unknown>> {
    return next(req).pipe(
      catchError((error: HttpErrorResponse) => {
        const domainError = new DomainError(
          error.error?.code ?? 'UNKNOWN',
          error.error?.message ?? 'An unexpected error occurred',
          error.status,
        );
        console.error(`[${domainError.code}] ${domainError.message}`);
        return throwError(() => domainError);
      }),
    );
  }
}
```

---

## 8. Testing Patterns

| Test type | Tool | What to test |
|-----------|------|-------------|
| Unit | Jest / Vitest | `domain/` entities, value objects |
| Component | Angular TestBed | `interfaces/` components |
| Service | TestBed + HttpTestingController | HTTP interactions |
| E2E | Playwright | Full user flows |

```typescript
// domain/entity.test.ts
describe('Order', () => {
  it('transitions from pending to confirmed', () => {
    const order = new Order();
    order.confirm();
    expect(order.status).toBe(OrderStatus.CONFIRMED);
  });
});
```

---

## 9. Anti-Patterns — NEVER DO

```typescript
// ❌ Business logic in components
export class OrderComponent {
  calculateTotal(items: Item[]): number { ... }  // BAD — belongs in domain/
}

// ❌ Subscribing in components without cleanup
ngOnInit() { this.service.orders$.subscribe(); }  // BAD — use async pipe

// ❌ Mutable state without Signals
public orders: Order[] = [];  // BAD — use signal()

// ❌ Direct HTTP calls in components
this.http.get('/api/orders');  // BAD — wrap in infrastructure/

// ❌ Domain entities with Angular decorators
export class Order {
  @Input() id: string;  // BAD — domain is Angular-free
}

// ❌ CSS in component TS
styles: [':host { display: block; }']  // BAD — use SCSS files
```

---

*Version: 1.1.0*
*Last updated: 2026-07-03*

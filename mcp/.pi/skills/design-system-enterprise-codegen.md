---
name: design-system-enterprise-codegen
description: Full reference for design system architecture with DDD + adapter pattern. Covers design tokens, CSS framework integration (Tailwind/MUI/Material/CSS Modules), component API contracts, theming, accessibility, and testing. Framework-agnostic — use with any component library.
---

# Design System & CSS Architecture — DDD with Adapter Pattern

> Canonical skill for building enterprise design systems using DDD principles.
> **Framework-agnostic** — works with Next.js, Angular, Vue, or any component framework.
> **CSS-agnostic** — works with Tailwind, MUI, Material, CSS Modules, or any styling approach.
>
> Use alongside `.pi/skills/agents/nextjs-codegen.md` or `.pi/skills/agents/angular-codegen.md`.

---

## The Core Idea: Domain → Infrastructure Adapter

A design system shouldn't be coupled to a specific CSS framework. The DDD layer structure enforces this:

```
domain/tokens/          ← Pure TypeScript: color values, spacing, typography
    │                     (zero CSS imports, zero framework imports)
    ▼
application/            ← Logic: variant resolution, theme switching
    │                     (pure functions, no rendering)
    ▼
infrastructure/         ← CSS ADAPTER: translates tokens into Tailwind/MUI/CSS Modules
    │                     (THIS is the only layer that knows about your CSS framework)
    ▼
interfaces/button/      ← Components: use the adapter, render the UI
```

| If you use... | Infrastructure adapter does | Components stay the same? |
|---------------|---------------------------|---------------------------|
| **Tailwind** | Generates `bg-primary text-white` class strings from tokens | ✅ Yes |
| **CSS Modules** | Generates `.module.css` with `var(--color-primary)` | ✅ Yes |
| **MUI** | Wraps MUI's `<Button>` component, maps tokens to MUI theme | ✅ Yes |
| **shadcn/ui** | Re-exports with token-based variants | ✅ Yes |
| **Styled Components** | Generates template literals | ✅ (but breaks RSC) |

---

## 1. Design System Structure (DDD Pattern)

```
shared/
  ui/
    domain/                             # Framework-free. Zero CSS imports.
      tokens/                           # Design tokens as typed value objects
        colors.ts                       # ColorToken — typed color palette
        typography.ts                   # TypeScale — font families, sizes
        spacing.ts                      # SpacingScale — margin, padding units
        shadows.ts                      # ShadowToken — elevation levels
        breakpoints.ts                  # Breakpoint — responsive breakpoints
      types.ts                          # Component API contracts
      component-api.ts                  # Base interfaces for all components
    application/                        # Logic layer (pure functions)
      variant-engine.ts                 # Variant resolution (cva-like)
      theme-service.ts                  # Dark/light theme switching
      token-resolver.ts                 # Token → value lookup
    infrastructure/                     # CSS ADAPTER — swap this per framework
      tailwind-adapter.ts               # Tailwind class generation from tokens
      css-modules-adapter.ts            # CSS Modules variable injection
      mui-adapter.ts                    # MUI theme provider wrapper
      material-adapter.ts              # Angular Material theme mapping
    interfaces/                         # Rendered components
      button/
        button.tsx                      # Component (imports from application + infrastructure)
        button.test.tsx                 # Tests
        button.stories.tsx              # Storybook stories
      card/
      modal/
      form/
```

---

## 2. Domain Layer — Design Tokens (Framework-Free)

This layer has zero CSS, zero framework, zero styling. It's pure TypeScript values.

```typescript
// domain/tokens/colors.ts
export class ColorToken {
  private constructor(
    public readonly name: string,
    public readonly value: string,        // Hex: '#0f766e'
    public readonly description: string,
  ) {}

  static readonly PRIMARY = new ColorToken('primary', '#0f766e', 'Primary brand color');
  static readonly PRIMARY_HOVER = new ColorToken('primary-hover', '#0d5e56');
  static readonly DESTRUCTIVE = new ColorToken('destructive', '#dc2626');
  static readonly BACKGROUND = new ColorToken('background', '#ffffff');
  static readonly FOREGROUND = new ColorToken('foreground', '#0f172a');

  // Dark mode overrides — still pure data, no CSS
  static readonly DARK: Record<string, string> = {
    background: '#0f172a',
    foreground: '#f8fafc',
  };

  static all(): ColorToken[] {
    return Object.values(this).filter(v => v instanceof ColorToken);
  }
}

// domain/tokens/spacing.ts
export class SpacingToken {
  constructor(
    public readonly name: string,
    public readonly value: string,     // CSS value: '0.25rem'
    public readonly px: number,         // Design value: 4
  ) {}

  static readonly XS = new SpacingToken('xs', '0.25rem', 4);
  static readonly SM = new SpacingToken('sm', '0.5rem', 8);
  static readonly MD = new SpacingToken('md', '1rem', 16);
  static readonly LG = new SpacingToken('lg', '1.5rem', 24);
  static readonly XL = new SpacingToken('xl', '2rem', 32);
}

// domain/tokens/typography.ts
export class TypeScale {
  constructor(
    public readonly name: string,
    public readonly fontSize: string,
    public readonly lineHeight: string,
    public readonly fontWeight: number,
  ) {}

  static readonly H1 = new TypeScale('h1', '2.5rem', '1.2', 700);
  static readonly H2 = new TypeScale('h2', '2rem', '1.3', 600);
  static readonly BODY = new TypeScale('body', '1rem', '1.5', 400);
  static readonly SMALL = new TypeScale('small', '0.875rem', '1.5', 400);
}
```

### Why tokens as domain objects?

- **Type-safe** — `ColorToken.PRIMARY.value` is a `string`, can't pass an invalid color
- **Documented** — each token has a `description` field, self-documenting
- **Enumerable** — `ColorToken.all()` can generate CSS variables, Tailwind config, or MUI theme
- **Framework-independent** — the exact same tokens feed Tailwind, CSS Modules, or MUI

---

## 3. Application Layer — Variant Engine

```typescript
// application/variant-engine.ts
export type VariantDefinition = {
  base: string;
  variants: Record<string, Record<string, string>>;
  defaults: Record<string, string>;
};

export function defineVariants(config: VariantDefinition) {
  return (props: Record<string, string>): string => {
    const classes = [config.base];
    for (const [key, values] of Object.entries(config.variants)) {
      const value = props[key] ?? config.defaults[key];
      if (value && values[value]) classes.push(values[value]);
    }
    return classes.filter(Boolean).join(' ');
  };
}
```

---

## 4. Infrastructure — CSS Framework Adapters

### Tailwind Adapter

```typescript
// infrastructure/tailwind-adapter.ts
// Translates domain tokens → Tailwind utility classes
import { ColorToken } from '../domain/tokens/colors';
import { SpacingToken } from '../domain/tokens/spacing';

export function tailwindColor(token: ColorToken): string {
  const map: Record<string, string> = {
    '#0f766e': 'bg-teal-700 text-white',
    '#dc2626': 'bg-red-600 text-white',
    '#ffffff': 'bg-white text-gray-900',
    '#0f172a': 'bg-slate-900 text-white',
  };
  return map[token.value] ?? '';
}

export function tailwindSpacing(token: SpacingToken): string {
  const map: Record<number, string> = {
    4: 'p-1', 8: 'p-2', 16: 'p-4', 24: 'p-6', 32: 'p-8',
  };
  return map[token.px] ?? '';
}
```

### CSS Modules Adapter

```typescript
// infrastructure/css-modules-adapter.ts
// Generates CSS custom properties from domain tokens
import { ColorToken } from '../domain/tokens/colors';
import { SpacingToken } from '../domain/tokens/spacing';

export function generateCSSVariables(): string {
  const colors = ColorToken.all().map(t =>
    `  --color-${t.name}: ${t.value};`
  ).join('\n');

  return `:root {\n${colors}\n}`;
}
```

### MUI Adapter

```typescript
// infrastructure/mui-adapter.ts
// Maps domain tokens to MUI theme structure
import { createTheme } from '@mui/material';
import { ColorToken } from '../domain/tokens/colors';

export function createMuiTheme() {
  return createTheme({
    palette: {
      primary: { main: ColorToken.PRIMARY.value },
      background: { default: ColorToken.BACKGROUND.value },
    },
  });
}
```

### Choosing an Adapter

| Adapter | When to use | Tradeoffs |
|---------|-------------|-----------|
| **Tailwind** | Startups, rapid prototyping, utility-first | Class strings get long; needs `clsx`/`tailwind-merge` |
| **CSS Modules** | Enterprise, strict design systems, component libraries | More boilerplate; full control over CSS |
| **MUI** | Need a complete component library out of the box | Bundle size; customization can be complex |
| **Material (Angular)** | Angular projects needing Material Design | Framework-locked; heavy |
| **shadcn/ui** | Next.js projects wanting Radix primitives with Tailwind | Requires Tailwind; new ecosystem |

**Rule:** Pick one adapter per project. Never mix. The adapter is the only file that changes when you switch CSS frameworks.

---

## 5. Component Implementation Pattern

```typescript
// interfaces/button/button.tsx
import { defineVariants } from '../../application/variant-engine';
//                                        ↑ import from application, not infrastructure
//                                        Components DON'T import the adapter directly

const buttonVariants = defineVariants({
  base: 'inline-flex items-center justify-center rounded-md transition-colors',
  variants: {
    variant: {
      primary: 'bg-primary text-white hover:bg-primary-hover',
      secondary: 'border border-primary text-primary hover:bg-primary/10',
      ghost: 'hover:bg-accent',
    },
    size: { sm: 'h-9 px-3', md: 'h-10 px-4', lg: 'h-11 px-8' },
  },
  defaults: { variant: 'primary', size: 'md' },
});

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'ghost';
  size?: 'sm' | 'md' | 'lg';
}

export function Button({ variant, size, className, ...props }: ButtonProps) {
  return (
    <button
      className={[buttonVariants({ variant, size }), className].filter(Boolean).join(' ')}
      {...props}
    />
  );
}
```

**Key rule:** Components import from `application/variant-engine.ts`, not from `infrastructure/`. The infrastructure adapter is only used at the app root (theme provider, CSS variable injection).

---

## 6. Theming (Dark Mode)

```typescript
// application/theme-service.ts
import { ColorToken } from '../domain/tokens/colors';

export type Theme = 'light' | 'dark';

export class ThemeService {
  private current: Theme = 'light';

  get theme(): Theme { return this.current; }

  setTheme(theme: Theme): void {
    this.current = theme;
    document.documentElement.setAttribute('data-theme', theme);
  }

  toggle(): void {
    this.setTheme(this.current === 'light' ? 'dark' : 'light');
  }
}
```

---

## 7. Testing — Framework-Independent

```typescript
// domain/tokens/colors.test.ts
describe('ColorToken', () => {
  it('all tokens have valid hex values', () => {
    for (const t of ColorToken.all()) {
      expect(t.value).toMatch(/^#[0-9a-fA-F]{6}$/);
    }
  });
});

// application/variant-engine.test.ts
describe('VariantEngine', () => {
  it('resolves variant classes', () => {
    const button = defineVariants({ base: 'btn', variants: { color: { red: 'bg-red' } }, defaults: { color: 'red' } });
    expect(button({})).toBe('btn bg-red');
  });
});
```

---

## 8. Choosing Your Stack — Quick Reference

| Project type | Recommended adapter | Why |
|-------------|-------------------|-----|
| Next.js app, small team | Tailwind + shadcn/ui | Fastest path, good defaults, RSC-compatible |
| Next.js app, large team | CSS Modules + custom tokens | Full control, strict design system |
| Angular app, Material look | Angular Material adapter | Native integration, theming built-in |
| Angular app, custom design | CSS Modules + SCSS tokens | Full control, SCSS features |
| Multi-framework design system | CSS Modules + CSS Custom Properties | Works everywhere, zero framework lock-in |

---

*Version: 1.1.0*
*Last updated: 2026-07-03*

# P0-A: Funciones como ciudadanos de primera clase — Análisis técnico

**Fecha:** 2026-04-21
**Estado:** análisis, pre-implementación

---

## 1. Estado actual del problema

### El síntoma

```zymbol
double(x) { <~ x * 2 }

f = double          // ❌ L5: "double" no existe como valor
[1,2,3]$> double    // ❌ L6: HOFs no aceptan funciones nombradas
```

### La causa raíz

Las funciones nombradas y los lambdas viven en **dos mundos separados** del intérprete:

| Aspecto | Función nombrada | Lambda |
|---------|-----------------|--------|
| Nodo AST | `Statement::FunctionDecl` | `Expr::Lambda` |
| Almacenamiento en runtime | `Interpreter.functions: HashMap<String, Rc<FunctionDef>>` | `scope_stack` como `Value::Function(FunctionValue)` |
| Búsqueda por identificador | `self.functions.get(name)` | `self.get_variable(name)` |
| Puede asignarse a variable | NO | SÍ |
| Puede pasarse a HOF | NO | SÍ |
| Scope al llamarse | Aislado (solo parámetros) | Capturas + parámetros |

Cuando el intérprete evalúa un `Expr::Identifier("double")`, **solo busca en el `scope_stack`**. Como las funciones nombradas nunca se insertan en el scope, `double` no existe como valor.

---

## 2. Archivos y líneas clave

### AST
- `crates/zymbol-ast/src/functions.rs` — `FunctionDecl` (líneas 29-36), `LambdaExpr` (líneas 12-27)
- `crates/zymbol-ast/src/lib.rs` — `Statement::FunctionDecl` (línea 113), `Expr::Lambda` (línea 209)

### Intérprete (tree-walker)
- `crates/zymbol-interpreter/src/lib.rs`
  - `Value` enum (líneas 139-154): una sola variante `Value::Function(FunctionValue)`
  - `FunctionValue.captures` = `Rc<HashMap<String,Value>>` — vacío para funciones, poblado para lambdas
  - Declaración de función nombrada: guarda en `self.functions` (línea ~840), **no en scope**
- `crates/zymbol-interpreter/src/functions_lambda.rs`
  - `eval_function_call`: busca primero en scope (lambda), luego en `self.functions` (función nombrada) — líneas 131-155
  - `eval_traditional_function_call`: crea scope aislado con `take_call_state()` — líneas 219-251
  - `eval_lambda_call`: restaura capturas antes de ejecutar — líneas 52-129
- `crates/zymbol-interpreter/src/collection_ops.rs`
  - HOFs (`$>`, `$?`, `$^`): hacen `eval_expr(&op.lambda)` y esperan `Value::Function` — líneas 474, 512, 562, 616

### VM / Compiler
- `crates/zymbol-bytecode/src/lib.rs` — instrucciones `MakeFunc(Reg, FuncIdx)` y `MakeClosure(Reg, FuncIdx, Vec<Reg>)`
- `crates/zymbol-vm/src/lib.rs` — `Value::Function(FuncIdx)` y `Value::Closure(FuncIdx, Rc<Vec<Value>>)`
- El compilador emite `Call(reg, FuncIdx, args)` para funciones nombradas (estático) y `CallDynamic` para valores

---

## 3. Decisión de diseño: ¿capturan scope las funciones como valores?

Esta es la decisión central antes de escribir una línea de código.

### Opción A — Capturan scope en el punto de uso (recomendada)

```zymbol
x = 10
adder(n) { <~ n + x }   // x no existe en scope de la función (aislada)

f = adder                // f captura el scope actual: { x: 10 }
f(5)                     // → 15
```

- Consistente con cómo funcionan los lambdas
- Consistente con Python, JS, Ruby, Rust, Swift
- **Rompe** la garantía actual de "funciones aisladas" solo cuando se usan **como valores**
- La función sigue siendo aislada cuando se llama directamente por nombre

### Opción B — No capturan scope, scope vacío al ejecutarse como valor

```zymbol
adder(n) { <~ n + x }
f = adder
f(5)                     // ❌ RuntimeError: x no definida
```

- Preserva el modelo actual de aislamiento total
- Inconsistente con lambdas → confusión garantizada
- L6 quedaría corregido pero sería difícil de usar en práctica

### Opción C — Capturan solo el scope de su definición (no el de uso)

```zymbol
x = 10
adder(n) { <~ n + x }   // x no estaba en scope de definición → error al crear el valor
```

- Más difícil de implementar (requiere rastrear scope en tiempo de declaración)
- Menos útil que la opción A

### Decisión recomendada: **Opción A**

Cuando una función nombrada se convierte en valor (`f = double`), captura el scope en ese momento, igual que un lambda. Semánticamente equivale a: `f = (x -> double(x))` pero sin la envoltura.

---

## 4. Cambios requeridos en el tree-walker

### 4.1 Evaluación de identificadores (cambio mínimo)

**Archivo:** `crates/zymbol-interpreter/src/expr_eval.rs` (o donde esté el match de `Expr::Identifier`)

**Cambio:** Cuando un identificador no se encuentra en el scope, buscar en `self.functions`. Si se encuentra, construir un `Value::Function` con las capturas del scope actual.

```rust
// Pseudocódigo del cambio
Expr::Identifier(ident) => {
    // 1. Buscar en scope (comportamiento actual)
    if let Some(val) = self.get_variable(&ident.name) {
        return Ok(val.clone());
    }
    // 2. NUEVO: buscar en funciones nombradas
    if let Some(func_def) = self.functions.get(&ident.name).cloned() {
        let captures = self.capture_current_scope(); // captura el scope actual
        return Ok(Value::Function(FunctionValue {
            params: func_def.parameters.iter().map(|p| p.name.clone()).collect(),
            body: LambdaBody::Block(func_def.body.clone()),
            captures: Rc::new(captures),
        }));
    }
    Err(/* variable not found */)
}
```

### 4.2 Ejecución de `Value::Function` con body Block

**Archivo:** `crates/zymbol-interpreter/src/functions_lambda.rs`, `eval_lambda_call`

**Problema:** `eval_lambda_call` ya maneja `LambdaBody::Block`, pero el bloque de una función nombrada tiene `<~` como retorno (return explícito), y eso ya funciona. Este cambio debería ser **cero o mínimo** si `LambdaBody::Block` ya maneja `ControlFlow::Return`.

**Verificar:** que `eval_lambda_call` con un `LambdaBody::Block` capture correctamente `ControlFlow::Return` de un `<~` explícito.

### 4.3 `$!!` en funciones-como-valor (bug L13)

Una vez que las funciones se llaman vía `eval_lambda_call` (porque son `Value::Function`), el `$!!` en un bloque-lambda ya debería funcionar **si** `eval_lambda_call` maneja `ControlFlow::Return`. Si no lo hace, ese es el fix adicional.

---

## 5. Cambios requeridos en el VM/Compiler

### 5.1 Nueva instrucción: `MakeFuncRef`

Cuando el compilador encuentra `Expr::Identifier("double")` y `double` es una función registrada, debe emitir una instrucción que cargue una referencia a esa función en un registro.

**Archivo:** `crates/zymbol-bytecode/src/lib.rs`

```rust
// Añadir instrucción
MakeFuncRef(Reg, FuncIdx),  // Carga referencia a función nombrada como valor
```

**Archivo:** `crates/zymbol-compiler/src/lib.rs`

Cuando se compila `Expr::Identifier(name)` y `name` no está en registros locales, verificar si es una función registrada y emitir `MakeFuncRef`.

### 5.2 Resolución en la VM

**Archivo:** `crates/zymbol-vm/src/lib.rs`

`MakeFuncRef` simplemente coloca `Value::Function(func_idx)` en el registro destino. La instrucción `CallDynamic` ya maneja `Value::Function`, por lo que el resto de la cadena (pasar a HOFs, pipe, etc.) debería funcionar sin más cambios en la VM.

---

## 6. HOFs — cambio requerido

**Ninguno.** Los HOFs ya hacen `eval_expr(&op.lambda)` y esperan `Value::Function`. Una vez que los identificadores de funciones nombradas puedan evaluarse a `Value::Function`, los HOFs funcionarán automáticamente.

---

## 7. Árbol de dependencias del cambio

```
[cambio mínimo]
eval_expr(Identifier) busca en self.functions
         │
         ▼
Value::Function con body=Block + captures=scope_actual
         │
         ├──→ f = double          ✅ (L5 resuelto)
         ├──→ arr$> double        ✅ (L6 resuelto)
         ├──→ 5 |> double(_)      ✅ (ya funciona, ahora también sin _)
         └──→ double(1)(2)        ✅ (funciones retornadas desde funciones)

[cambio VM]
MakeFuncRef(Reg, FuncIdx) en compiler
         │
         ▼
Value::Function(FuncIdx) en registro → CallDynamic ya existente
```

---

## 8. Casos de prueba necesarios

```zymbol
// T1: asignación básica
double(x) { <~ x * 2 }
f = double
f(5)                         // → 10

// T2: HOF map
double(x) { <~ x * 2 }
[1,2,3]$> double             // → [2,4,6]

// T3: HOF filter
positive(x) { <~ x > 0 }
[-1,2,-3,4]$? positive       // → [2,4]

// T4: pipe sin _ (depende de P2-B, pero verificar que no rompa)
5 |> double(_)               // → 10 (comportamiento actual, debe seguir funcionando)

// T5: función retornada desde función
make_adder(n) { <~ x -> x + n }
add5 = make_adder(5)
add5(3)                      // → 8

// T6: función pasada y llamada desde módulo
// (requiere verificar interacción con sistema de módulos)

// T7: función nombrada en variable, scope aislado preservado cuando se llama directo
x = 99
uses_x(n) { <~ n }          // x no accesible directamente (scope aislado en llamada directa)
uses_x(1)                    // → 1 (no accede a x del scope externo)

// T8: función nombrada como valor SÍ captura scope
x = 10
adder(n) { <~ n + x }       // cuando se usa como valor, captura x
f = adder                    // captura { x: 10 }
f(5)                         // → 15 (Opción A — si se decide así)
```

---

## 9. Riesgos y efectos secundarios

| Riesgo | Probabilidad | Mitigación |
|--------|-------------|------------|
| Llamada directa a función nombrada cambia comportamiento | Baja — el código de `eval_function_call` ya prioriza el scope antes de `self.functions`; si un lambda y una función tienen el mismo nombre, gana el lambda (comportamiento actual) | No hay cambio en la ruta de llamada directa |
| Ciclos de captura (función que se captura a sí misma) | Media | Usar `Rc` ya existente; si hay ciclos, el `Rc` no los rompe — documentar limitación |
| Overhead de captura en cada uso como valor | Baja | La copia del scope ocurre solo cuando se convierte a valor, no en cada llamada |
| VM: funciones anidadas con MakeFuncRef en closures | Media | Verificar que `FuncIdx` sea estable entre el compiler y el VM para funciones anidadas |

---

## 10. Orden de implementación recomendado

1. **Tree-walker primero** — es el intérprete canónico
   - Modificar `eval_expr(Identifier)` para resolver funciones nombradas como valores
   - Verificar que `eval_lambda_call` con `LambdaBody::Block` maneja `<~` (ControlFlow::Return)
   - Ejecutar los 8 casos de prueba

2. **Tests de regresión** — correr suite completa para detectar colisiones de nombres

3. **VM después** — agregar `MakeFuncRef` al bytecode y al compiler

4. **Documentar la decisión de scope** (Opción A) en MANUAL.md §9

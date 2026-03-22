  Plan: Quick Wins + Bytecode VM para ~1x Python

  Estrategia recomendada

  QW (1 sesión)  →  Sprint 4A (2 sesiones)  →  Sprint 4B (2 sesiones)  →  Sprint 4C (1 sesión)
     ~3.5x recursion    ~800ms recursion          ~350ms recursion          tests + default

  Por qué empezar con QW antes de VM:
  Los QW eliminan bugs de diseño actuales (double scope, lazy HashMap). Algunos de esos fixes también benefician la infraestructura del VM (el tree-walker sigue como fallback). Son
  reversibles, rápidos y de bajo riesgo.

  ---
  Fase 0 — Quick Wins (1 sesión)

  Objetivo: Eliminar overhead evitable sin cambio arquitectural.

  QW1 — execute_block_no_scope() para cuerpos de función

  Problema: Cada función hace doble scope push:
  take_call_state()    →  scope[0] = {n: 30}  ← params aquí
  execute_block(body)  →  scope[1] = {}        ← vars locales aquí
  get_variable("n") busca en scope[1] primero (miss), luego scope[0] (hit). Doble overhead.

  Fix: Añadir execute_block_no_scope() que ejecuta statements sin push/pop. Llamarlo en eval_traditional_function_call y eval_lambda_call.
  fn execute_block_no_scope(&mut self, block: &Block) -> Result<()> {
      for stmt in &block.statements {
          self.execute_statement(stmt)?;
          if self.control_flow != ControlFlow::None { break; }
      }
      Ok(())
  }
  Impacto: ~130ns × 2.69M calls = −350ms en recursion.

  ---
  QW2 — updates HashMap lazy

  Problema: let mut updates = HashMap::new() se crea en cada función aunque el 99% no tiene output params (<~).
  // ACTUAL: siempre aloca
  let mut updates = HashMap::new();   // ← 2.69M HashMaps en fib(30)
  Fix:
  let has_output_params = func_def.parameters.iter().any(|p| matches!(p.kind, ParameterKind::Output));
  if has_output_params {
      // solo aquí crear el HashMap
  }
  Impacto: −108ms en recursion.

  ---
  QW3 — Pool para mutable_vars_stack y const_vars_stack Vecs

  Problema: take_call_state() crea dos nuevos Vec con vec![...]:
  self.mutable_vars_stack = vec![self.mut_set_pool.pop().unwrap_or_default()];  // ← new Vec
  self.const_vars_stack   = vec![self.const_set_pool.pop().unwrap_or_default()]; // ← new Vec
  El scope_vec_pool ya existe para scope_stack. Falta hacer lo mismo para los otros dos.

  Fix: Añadir mut_vec_pool: Vec<Vec<HashSet<String>>> y const_vec_pool al struct. Reciclar en restore_call_state.
  Impacto: −161ms en recursion.

  ---
  QW4 — arg_values con Vec::with_capacity

  Problema: let mut arg_values = Vec::new() siempre reserva 0 y rehashea.

  Fix: Vec::with_capacity(arguments.len()) — una sola alloc.
  Impacto: menor, ~15ms en match/stress.

  ---
  QW5 — #[inline(always)] en el hot path

  Las funciones llamadas millones de veces deben ser inlined por el compilador:
  #[inline(always)] fn push_scope(&mut self) { ... }
  #[inline(always)] fn pop_scope(&mut self) { ... }
  #[inline(always)] fn get_variable(&self, name: &str) -> Option<&Value> { ... }
  #[inline(always)] fn set_variable(&mut self, name: &str, value: Value) { ... }
  #[inline(always)] fn is_const(&self, name: &str) -> bool { ... }
  Impacto: ~50ns × 2.69M = −135ms (elimina call overhead de funciones pequeñas).

  ---
  QW6 — control_flow check con bool flag

  Problema: self.control_flow != ControlFlow::None ejecuta un PartialEq sobre un enum con Option<Value> adentro — clone implícito en algunos paths.

  Fix: Añadir has_control_flow: bool (análogo a has_any_const):
  #[inline(always)]
  fn has_pending_control_flow(&self) -> bool {
      self.has_control_flow  // bool check, cero overhead
  }
  Impacto: ~20ns × 5M checks = −100ms global.

  ---
  Resultado esperado QW

  ┌───────────┬────────────────┬────────────────┬─────────┐
  │ Benchmark │    Antes QW    │   Después QW   │ Mejora  │
  ├───────────┼────────────────┼────────────────┼─────────┤
  │ recursion │ 4561ms (11.9x) │ ~3100ms (~8x)  │ −1461ms │
  ├───────────┼────────────────┼────────────────┼─────────┤
  │ stress    │ 495ms (3.9x)   │ ~380ms (~3x)   │ −115ms  │
  ├───────────┼────────────────┼────────────────┼─────────┤
  │ match     │ 333ms (3.3x)   │ ~240ms (~2.4x) │ −93ms   │
  └───────────┴────────────────┴────────────────┴─────────┘

  ---
  Fase 1 — Sprint 4A: Bytecode VM Foundation (2 sesiones)

  Objetivo: Compilador AST→bytecode + VM executor para expresiones, variables y loops.

  Arquitectura elegida: Register VM (no Stack VM)

  Por qué Register VM sobre Stack VM:

  ┌─────────────────┬─────────────────────────┬─────────────────────────────────────────┐
  │                 │        Stack VM         │               Register VM               │
  ├─────────────────┼─────────────────────────┼─────────────────────────────────────────┤
  │ Variables       │ push/pop explicit       │ registers[index] directo                │
  ├─────────────────┼─────────────────────────┼─────────────────────────────────────────┤
  │ Variable access │ O(stack depth)          │ O(1) — array index                      │
  ├─────────────────┼─────────────────────────┼─────────────────────────────────────────┤
  │ Implementación  │ más simple              │ ligeramente más compleja                │
  ├─────────────────┼─────────────────────────┼─────────────────────────────────────────┤
  │ Performance     │ ~2-3x sobre tree-walker │ ~5-8x sobre tree-walker                 │
  ├─────────────────┼─────────────────────────┼─────────────────────────────────────────┤
  │ Python usa      │ Stack VM (CPython)      │ —                                       │
  ├─────────────────┼─────────────────────────┼─────────────────────────────────────────┤
  │ Lua 5 usa       │ —                       │ Register VM (3-4x más rápido que Lua 4) │
  └─────────────────┴─────────────────────────┴─────────────────────────────────────────┘

  Para Zymbol, un Register VM con frames de tamaño fijo (calculado en compile time) es el camino correcto.

  ---
  Diseño de opcodes (45 instrucciones)

  // crates/zymbol-bytecode/src/lib.rs
  #[derive(Debug, Clone)]
  pub enum Instruction {
      // ── Carga de valores ────────────────────────────────────────────────
      LoadInt(Reg, i64),           // reg = i64 literal
      LoadFloat(Reg, f64),         // reg = f64 literal
      LoadBool(Reg, bool),         // reg = bool literal
      LoadStr(Reg, StrIdx),        // reg = string_pool[idx]
      LoadChar(Reg, char),         // reg = char literal
      LoadUnit(Reg),               // reg = ()

      // ── Variables ───────────────────────────────────────────────────────
      Move(Reg, Reg),              // dst = src
      LoadGlobal(Reg, GlobalIdx),  // reg = globals[idx]
      StoreGlobal(GlobalIdx, Reg), // globals[idx] = reg

      // ── Aritmética ──────────────────────────────────────────────────────
      AddInt(Reg, Reg, Reg),       // dst = a + b  (Int)
      SubInt(Reg, Reg, Reg),
      MulInt(Reg, Reg, Reg),
      DivInt(Reg, Reg, Reg),
      ModInt(Reg, Reg, Reg),
      AddFloat(Reg, Reg, Reg),
      SubFloat(Reg, Reg, Reg),
      MulFloat(Reg, Reg, Reg),
      AddStr(Reg, Reg, Reg),       // string concatenation

      // ── Comparación ─────────────────────────────────────────────────────
      CmpEq(Reg, Reg, Reg),        // dst = a == b
      CmpLt(Reg, Reg, Reg),        // dst = a < b
      CmpLe(Reg, Reg, Reg),
      CmpGt(Reg, Reg, Reg),
      CmpGe(Reg, Reg, Reg),
      CmpNe(Reg, Reg, Reg),

      // ── Control flow ────────────────────────────────────────────────────
      Jump(Label),                 // pc = label
      JumpIf(Reg, Label),          // if reg: pc = label
      JumpIfNot(Reg, Label),       // if !reg: pc = label

      // ── Llamadas a función ───────────────────────────────────────────────
      Call(Reg, FuncIdx, &[Reg]),  // dst = functions[idx](args...)
      CallDyn(Reg, Reg, &[Reg]),   // dst = reg_func(args...)
      Return(Reg),                 // return reg

      // ── I/O ─────────────────────────────────────────────────────────────
      Print(Reg),                  // >> reg
      PrintNewline,                // ¶

      // ── Arrays ──────────────────────────────────────────────────────────
      NewArray(Reg, u16),          // reg = [] con capacity hint
      ArrayPush(Reg, Reg),         // arr.push(val) in-place
      ArrayGet(Reg, Reg, Reg),     // dst = arr[idx]
      ArraySet(Reg, Reg, Reg),     // arr[idx] = val
      ArrayLen(Reg, Reg),          // dst = arr.len()

      // ── Pattern match ───────────────────────────────────────────────────
      MatchInt(Reg, i64, Label),   // if reg == int: jump label
      MatchRange(Reg, i64, i64, Label), // if start <= reg <= end: jump
      MatchStr(Reg, StrIdx, Label),

      // ── Especiales ──────────────────────────────────────────────────────
      Nop,
      Halt,
  }

  pub type Reg = u8;       // hasta 256 registros por frame
  pub type Label = u32;    // offset en el bytecode del chunk
  pub type FuncIdx = u32;  // índice en la tabla de funciones
  pub type GlobalIdx = u32; // índice en globals array
  pub type StrIdx = u32;    // índice en string pool

  ---
  Estructura de un Chunk (unidad compilable)

  pub struct Chunk {
      pub name: String,
      pub instructions: Vec<Instruction>,
      pub constants: Vec<Value>,       // pool de strings y floats
      pub num_registers: u8,           // calculado en compile time
      pub num_params: u8,
      pub labels: Vec<u32>,            // label → instruction offset
  }

  pub struct CompiledProgram {
      pub main: Chunk,
      pub functions: Vec<Chunk>,       // indexed by FuncIdx
      pub globals: Vec<Value>,         // initial values of globals
      pub string_pool: Vec<String>,
  }

  ---
  VM Executor — frame sin HashMap

  pub struct CallFrame {
      pub registers: Vec<Value>,    // [0..num_registers) — array CONTIGUO
      pub return_reg: Reg,          // dónde poner el return value
      pub chunk_idx: usize,         // qué función estamos ejecutando
      pub ip: usize,                // instruction pointer
  }

  pub struct VM {
      frames: Vec<CallFrame>,       // call stack (max depth ~1000)
      globals: Vec<Value>,          // global variables por índice
      output: Box<dyn Write>,
  }

  Variable lookup en VM: frame.registers[reg_idx] — 1 array access, O(1), cache-friendly.
  vs tree-walker: get_variable("n") — hash + iterate Vec, O(D×H).

  ---
  Compiler — mapear variables a registros

  pub struct Compiler {
      register_map: HashMap<String, Reg>,  // solo en compile time
      next_reg: u8,
      instructions: Vec<Instruction>,
      // ...
  }

  impl Compiler {
      fn alloc_reg(&mut self, name: &str) -> Reg {
          let reg = self.next_reg;
          self.register_map.insert(name.to_string(), reg);
          self.next_reg += 1;
          reg
      }

      fn get_reg(&self, name: &str) -> Reg {
          *self.register_map.get(name).expect("undefined var")
      }

      fn compile_expr(&mut self, expr: &Expr) -> Result<Reg> {
          match expr {
              Expr::Literal(lit) => {
                  let dst = self.alloc_tmp();
                  match &lit.value {
                      Literal::Int(n) => self.emit(Instruction::LoadInt(dst, *n)),
                      // ...
                  }
                  Ok(dst)
              }
              Expr::Identifier(ident) => {
                  Ok(self.get_reg(&ident.name))  // cero instrucción: ya está en su reg
              }
              Expr::Binary(bin) => {
                  let lhs = self.compile_expr(&bin.left)?;
                  let rhs = self.compile_expr(&bin.right)?;
                  let dst = self.alloc_tmp();
                  match bin.op {
                      BinaryOp::Add => self.emit(Instruction::AddInt(dst, lhs, rhs)),
                      // ...
                  }
                  Ok(dst)
              }
              // ...
          }
      }
  }

  ---
  Fase 2 — Sprint 4B: Function Calls + Control Flow (2 sesiones)

  Objetivo: Llamadas a función completas con frames, match, loops con ranges.

  Function call en VM — sin HashMap, O(1)

  CALL instruction:
  1. Nueva CallFrame con registers[0..num_params] = args (Move, sin alloc de HashMap)
  2. Push frame al call stack (Vec::push — amortized O(1))
  3. Continuar ejecución del chunk de la función

  RETURN instruction:
  1. return_value = registers[return_reg]
  2. Pop frame del call stack
  3. registers[caller.return_reg] = return_value
  4. Continuar ejecución del caller

  Costo por call: ~50-100ns (vs ~1700ns en tree-walker). Target: fib(30) < 500ms.

  ---
  Fase 3 — Sprint 4C: Integración completa (1-2 sesiones)

  Objetivo: VM cubre el 100% del lenguaje, pasa todos los tests.

  Etapas

  1. Opcodes restantes: colecciones HOF (map/filter/reduce), módulos, error handling, lambdas
  2. Flag --vm en CLI: ejecutar con VM como opt-in primero
  3. Test suite completo: los 523 tests deben pasar en modo VM
  4. VM como default: una vez 523 tests pasan, VM reemplaza tree-walker

  ---
  Resumen ejecutivo: orden de implementación

  Sesión 1:  QW1-QW6     → bugs de diseño eliminados      →  recursion ~3100ms
  Sesión 2:  4A parte 1  → Chunk + opcodes + Compiler básico (lit, var, aritmética)
  Sesión 3:  4A parte 2  → VM executor + loops + if/else  →  stress/match primero
  Sesión 4:  4B parte 1  → CALL/RETURN + frames           →  recursion ~800ms
  Sesión 5:  4B parte 2  → MATCH + arrays + strings       →  todos mejoran
  Sesión 6:  4C          → HOF + módulos + test suite     →  523 tests en VM

  ---
  Proyección final con VM completa

  ┌─────────────┬────────────┬────────────────┬───────────────┬───────────────┬──────────────────┐
  │  Benchmark  │ Python avg │     Hoy S3     │    Tras QW    │    Tras 4A    │    Tras 4B/C     │
  ├─────────────┼────────────┼────────────────┼───────────────┼───────────────┼──────────────────┤
  │ recursion   │ 383ms      │ 4561ms (11.9x) │ ~3100ms (8x)  │ ~800ms (2x)   │ ~350ms (0.9x) ✅ │
  ├─────────────┼────────────┼────────────────┼───────────────┼───────────────┼──────────────────┤
  │ stress      │ 128ms      │ 495ms (3.9x)   │ ~380ms (3x)   │ ~200ms (1.6x) │ ~130ms (1x) ✅   │
  ├─────────────┼────────────┼────────────────┼───────────────┼───────────────┼──────────────────┤
  │ match       │ 100ms      │ 333ms (3.3x)   │ ~240ms (2.4x) │ ~120ms (1.2x) │ ~100ms (1x) ✅   │
  ├─────────────┼────────────┼────────────────┼───────────────┼───────────────┼──────────────────┤
  │ collections │ 72ms       │ 65ms (0.9x ✅) │ ~60ms         │ ~50ms         │ ~45ms (0.6x) ✅  │
  ├─────────────┼────────────┼────────────────┼───────────────┼───────────────┼──────────────────┤
  │ strings     │ 39ms       │ 33ms (0.8x ✅) │ ~30ms         │ ~28ms         │ ~25ms (0.6x) ✅  │
  └─────────────┴────────────┴────────────────┴───────────────┴───────────────┴──────────────────┘

  ---
  Recomendación

  Empezar con QW esta sesión — son correcciones de bugs, no overhead técnico. Luego en la siguiente sesión arrancar el diseño del VM con los opcodes y el Compiler básico. La VM es el cambio
  que lleva Zymbol de "intérprete experimental" a "lenguaje con rendimiento competitivo".
---------------------------------------------------------------------------------------------------------------------------------------
  ┌───────────────────┬────────────┬────────────┬────────────┬────────────┬─────────────┐
  │     Benchmark     │ Zymbol min │ Zymbol avg │ Python min │ Python avg │ Ratio (avg) │
  ├───────────────────┼────────────┼────────────┼────────────┼────────────┼─────────────┤
  │ stress            │ 501ms      │ 596ms      │ 152ms      │ 164ms      │ 3.6x        │
  ├───────────────────┼────────────┼────────────┼────────────┼────────────┼─────────────┤
  │ bench_match       │ 293ms      │ 359ms      │ 112ms      │ 132ms      │ 2.7x        │
  ├───────────────────┼────────────┼────────────┼────────────┼────────────┼─────────────┤
  │ bench_recursion   │ 4062ms     │ 4475ms     │ 388ms      │ 445ms      │ 10.1x       │
  ├───────────────────┼────────────┼────────────┼────────────┼────────────┼─────────────┤
  │ bench_collections │ 72ms       │ 97ms       │ 86ms       │ 108ms      │ 0.9x ✅     │
  ├───────────────────┼────────────┼────────────┼────────────┼────────────┼─────────────┤
  │ bench_strings     │ 31ms       │ 50ms       │ 42ms       │ 49ms       │ ~1x ✅      │
  └───────────────────┴────────────┴────────────┴────────────┴────────────┴─────────────┘

  Nota: los tiempos de Python incluyen ~40ms de startup del proceso en cada run (overhead de python3 invocation), lo que infla los promedios de Python. Con startup descontado:

  ┌─────────────┬───────────────────────────────────────────────┐
  │  Benchmark  │ Ratio real (descontando ~40ms startup Python) │
  ├─────────────┼───────────────────────────────────────────────┤
  │ stress      │ ~3.9x                                         │
  ├─────────────┼───────────────────────────────────────────────┤
  │ bench_match │ ~3.0x                                         │
  ├─────────────┼───────────────────────────────────────────────┤
  │ recursion   │ ~10.7x                                        │
  ├─────────────┼───────────────────────────────────────────────┤
  │ collections │ ~0.75x — Zymbol más rápido ✅                 │
  ├─────────────┼───────────────────────────────────────────────┤
  │ strings     │ ~1.0x — paridad ✅                            │
  └─────────────┴───────────────────────────────────────────────┘

  Collections y Strings ya están por debajo o en paridad con Python. El cuello de botella restante es fundamentalmente el overhead de function call (recursion 10x) y los loops con scope push/pop (stress/match 3-4x) — lo que
  resuelve la VM en Sprint 4.

---------------------------------------------------------------------------------------------------------------------------------------
    Diagnóstico

  El 10.1x vs Python viene de dos fuentes distintas:

  fib(30) — 12.9x

  Cada una de las 2.69M llamadas hace un execute_block innecesario para la rama ? n <= 1 { <~ n } o _ { ... }. Ese bloque no define variables nuevas — solo hace <~ — pero igual hace push/pop scope. Esto no lo cubría QW1 (que solo
   eliminó el scope del body de la función, no el de las ramas if/else).

  ackermann(3,6) — 17.1x

  Más pronunciado porque tiene 2 parámetros (doble binding) + 3 ramas if/else-if/else (3 posibles execute_block), cada una también con push/pop de scope innecesario.

  QW7 — lo que se puede hacer ahora

  execute_block_no_scope para ramas if/else cuando el bloque no tiene Assignment ni ConstDecl:

  // if_stmt.rs — check en O(stmts) antes de ejecutar
  fn needs_scope(block: &Block) -> bool {
      block.statements.iter().any(|s|
          matches!(s, Statement::Assignment(_) | Statement::ConstDecl(_)))
  }

  // Luego en execute_if:
  if needs_scope(&if_stmt.then_block) {
      self.execute_block(&if_stmt.then_block)?;
  } else {
      self.execute_block_no_scope(&if_stmt.then_block)?;
  }

  Impacto estimado: −350ms en fib(30), recursion pasaría de ~4475ms (10.1x) a ~3800ms (~8.5x).

  Después de ese fix, el resto del overhead (~350ns/call de "misc") es el piso del tree-walker — solo la VM lo elimina. ¿Implementamos QW7 ahora?

---------------------------------------------------------------------------------------------------------------------------------------
  ┌─────────────┬───────────┬─────────────┬────────────────┬────────────┬─────────┐
  │  Benchmark  │ Antes QW7 │ Después QW7 │     Mejora     │ Python avg │  Ratio  │
  ├─────────────┼───────────┼─────────────┼────────────────┼────────────┼─────────┤
  │ recursion   │ 4475ms    │ 3234ms      │ −1241ms (−28%) │ 358ms      │ 9.0x    │
  ├─────────────┼───────────┼─────────────┼────────────────┼────────────┼─────────┤
  │ stress      │ 495ms     │ 355ms       │ −140ms (−28%)  │ 137ms      │ 2.6x    │
  ├─────────────┼───────────┼─────────────┼────────────────┼────────────┼─────────┤
  │ bench_match │ 333ms     │ 256ms       │ −77ms (−23%)   │ 85ms       │ 3.0x    │
  ├─────────────┼───────────┼─────────────┼────────────────┼────────────┼─────────┤
  │ collections │ 65ms      │ 65ms        │ sin cambio     │ 88ms       │ 0.7x ✅ │
  ├─────────────┼───────────┼─────────────┼────────────────┼────────────┼─────────┤
  │ strings     │ 33ms      │ 36ms        │ varianza       │ 33ms       │ ~1x ✅  │
  └─────────────┴───────────┴─────────────┴────────────────┴────────────┴─────────┘

  QW7 fue el cambio con mayor impacto de todos los Quick Wins. El needs_own_scope() eliminó el push/pop scope de cada rama if/else que no declara variables — lo que cubre el 100% de los casos en funciones recursivas.

  Ahora commit y push, y listo para Sprint 4 VM:

  ● ---
  Respuesta directa

  Sí hay 3 mejoras concretas, pero tienen un techo duro:

  QW8 — Eliminar clone innecesario en param binding (1 línea)

  // ACTUAL: clona el valor al insertar en el scope
  for (param, arg_value) in func_def.parameters.iter().zip(arg_values.iter()) {
      self.set_variable(&param.name, arg_value.clone()); // ← CLONE innecesario
  }

  // FIX: mover en lugar de clonar
  for (param, arg_value) in func_def.parameters.iter().zip(arg_values.into_iter()) {
      self.set_variable(&param.name, arg_value); // ← MOVE
  }
  Ahorro: −135ms en fib(30).

  QW9 — Pool para arg_values Vec (igual que B10 hizo con scopes)

  La Vec que almacena los argumentos evaluados se crea y destruye en cada call. Con pool =
  −67ms.

  QW10 — SmallVec lineal en lugar de HashMap para scopes pequeños

  Para funciones con ≤4 variables (que es el 100% de los benchmarks), una búsqueda lineal
  sobre un array contiguo es más rápida que HashMap porque elimina el hash computation. −390ms
   en los 13M lookups de fib.

  ---
  Pero el techo es infranqueable sin VM

  ┌───────────────────────────┬────────────────────────┬─────────────────────────────────┐
  │        Componente         │         Costo          │           Eliminable            │
  ├───────────────────────────┼────────────────────────┼─────────────────────────────────┤
  │ take_call_state + restore │ 350ns = 942ms del      │ ❌ NO — es la arquitectura      │
  │                           │ total                  │ misma                           │
  ├───────────────────────────┼────────────────────────┼─────────────────────────────────┤
  │ dispatch eval_expr 37     │ ~100ns/call            │ ❌ NO — es el tree-walker       │
  │ arms                      │                        │                                 │
  ├───────────────────────────┼────────────────────────┼─────────────────────────────────┤
  │ QW8 clone en params       │ 50ns → 0               │ ✅                              │
  ├───────────────────────────┼────────────────────────┼─────────────────────────────────┤
  │ QW9 Vec arg_values        │ 25ns → 0               │ ✅                              │
  ├───────────────────────────┼────────────────────────┼─────────────────────────────────┤
  │ QW10 HashMap → linear     │ 30ns → 5ns             │ ✅ parcial                      │
  └───────────────────────────┴────────────────────────┴─────────────────────────────────┘

  Con QW8+QW9+QW10: fib(30) iría de 1.5s → ~1.2s (~20% mejora). Útil pero no dramático.

  El 60% del overhead es take_call_state + restore_call_state — el acto de guardar/restaurar
  el estado del intérprete para aislar cada llamada. Eso es el tree-walker. No hay forma de
  optimizarlo sin reemplazarlo.

  ¿Los implementamos? Son cambios pequeños y seguros, pero el retorno es modesto (~20%). Si el
   objetivo es llegar a ~Python, solo la VM da ese salto.

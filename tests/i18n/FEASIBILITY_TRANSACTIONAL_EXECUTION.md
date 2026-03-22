# Informe de Factibilidad: Ejecución Transaccional de Archivos .zy

**Fecha**: 2025-12-29
**Estado**: Análisis Completo

---

## 📋 Requisitos del Usuario

### Sintaxis Propuesta:

1. **Ejecución directa con output impreso**:
   ```zymbol
   </ app_test.zy />
   ```
   Ejecuta el archivo y imprime la salida directamente en pantalla.

2. **Ejecución con captura de output**:
   ```zymbol
   retorno = </ app_test.zy />
   ```
   Ejecuta el archivo y captura la salida en una variable (sin imprimir).

3. **Acceso a parámetros de línea de comandos**:
   ```zymbol
   ><PARAMETRO
   ```
   Captura los argumentos CLI en la variable especificada (usuario elige el nombre):
   ```bash
   zymbol app_test.zy --version
   # Después de ><PARAMETRO
   # PARAMETRO = ["--version"]
   ```

   **Filosofía**: Operador simbólico `><` + nombre elegido por usuario (español, inglés, coreano, etc.)

---

## ✅ Factibilidad Técnica: ALTA

### Fundamentos Existentes

La implementación es **ALTAMENTE FACTIBLE** porque la infraestructura necesaria ya existe:

#### 1. **Captura de Output (Ya Implementado)** ✅

**Ubicación**: `crates/zymbol-interpreter/src/lib.rs:205-216, 329`

El intérprete ya tiene un mecanismo completo de captura de output:

```rust
/// Create interpreter with custom output writer
pub fn with_output(output: W) -> Self {
    Self {
        output,
        variables: HashMap::new(),
        // ... resto de campos
    }
}
```

**Uso actual en módulos** (línea 329):
```rust
// Create a new interpreter for the module with a buffer to capture output
let mut module_interp = Interpreter::with_output(Vec::new());
```

**Conclusión**: Ya se puede capturar output en memoria usando `Vec<u8>` como Writer. ✅

#### 2. **Carga y Ejecución de Archivos (Ya Implementado)** ✅

**Ubicación**: `crates/zymbol-interpreter/src/lib.rs:294-326`

Ya existe la función `load_module()` que:
- Lee archivos desde disco
- Tokeniza y parsea el código
- Ejecuta el programa en un intérprete aislado

**Código existente**:
```rust
fn load_module(&mut self, file_path: &Path) -> Result<()> {
    // Read file
    let source = std::fs::read_to_string(file_path)?;

    // Lex and parse
    let lexer = Lexer::new(&source, FileId(0));
    let (tokens, lex_diagnostics) = lexer.tokenize();
    let parser = Parser::new(tokens);
    let program = parser.parse()?;

    // Execute in isolated interpreter
    let mut module_interp = Interpreter::with_output(Vec::new());
    // ...
}
```

**Conclusión**: La lógica de carga y ejecución ya está implementada. Solo necesitamos adaptarla. ✅

---

## ⚠️ Desafíos Identificados

### ✅ Solución: Sintaxis Simbólica `><variable`

**Problema Original**: El token `<>` ya existe como operador `Neq` (not equal), causando conflicto.

**Solución Adoptada**: Usar `><` (inverso visual) para captura de parámetros CLI.

**Ventajas**:
- ✅ **Simbólico puro**: No usa palabras en inglés (mantiene filosofía language-agnostic)
- ✅ **Sin conflictos**: `><` no existe en el lexer actual
- ✅ **Coherente**: Sigue el patrón de otros operadores (`>>`, `<<`, `<#`, `<~`)
- ✅ **Flexibilidad**: Usuario elige el nombre de variable en su idioma

**Sintaxis**:
```zymbol
><parametros      // Español
><args            // Inglés
><파라미터          // Coreano
><παράμετροι      // Griego
><פרמטרים         // Hebreo

// Después funciona como variable normal
? parametros$# > 0 { ... }
```

**Token Necesario**: `CliArgsCapture` → `><`

---

## 🔧 Diseño de Implementación

### Características a Implementar:

#### 1. Ejecución Transaccional: `</ path />`

**Nuevos Tokens Requeridos**:
- `ExecuteStart` → `</`
- `ExecuteEnd` → `/>`

**Ubicación de Implementación**: `crates/zymbol-lexer/src/lib.rs` (después de línea 354)

```rust
// Check for </ (execute start)
if ch == '<' && self.peek() == Some('/') {
    self.advance();
    self.advance();
    return Token::new(TokenKind::ExecuteStart, self.span(start));
}
```

**AST Node Nuevo** (`crates/zymbol-ast/src/lib.rs`):
```rust
pub struct ExecuteExpr {
    pub path: String,    // Path del archivo a ejecutar
}
```

**Parser**: Reconocer `</ "path" />` como expresión

**Interpreter**:
- Caso 1 (statement): `</ file.zy />` → ejecutar y imprimir output
- Caso 2 (expression): `x = </ file.zy />` → ejecutar y capturar output en string

#### 2. Parámetros CLI: Operador `><variable`

**Nuevo Token Requerido**:
- `CliArgsCapture` → `><`

**Ubicación de Implementación**: `crates/zymbol-lexer/src/lib.rs` (después de línea 347)

```rust
// Check for >< (CLI args capture)
if ch == '>' && self.peek() == Some('<') {
    self.advance();
    self.advance();
    return Token::new(TokenKind::CliArgsCapture, self.span(start));
}
```

**AST Node Nuevo** (`crates/zymbol-ast/src/lib.rs`):
```rust
pub struct CliArgsCaptureStmt {
    pub variable_name: String,  // Nombre de variable elegido por usuario
}
```

**Parser**: Reconocer `><identifier` como statement

**Interpreter**:
- Al ejecutar `><variable`, tomar los argumentos CLI del contexto
- Asignar array de strings a la variable especificada
- Variable funciona normalmente después

**Uso en código Zymbol**:
```zymbol
// Capturar argumentos CLI
><args

// Usar como variable normal
@ arg:args {
    >> "Argumento: " arg ¶
}

? args$# > 0 {
    primer_arg = args[0]
    ? primer_arg == "--version" {
        >> "Zymbol 1.0.0" ¶
    }
}
```

---

## 📊 Complejidad de Implementación

| Componente | Complejidad | Justificación |
|------------|-------------|---------------|
| Lexer (Tokens `</`, `/>`, `><`) | **Baja** | Solo agregar 3 tokens al patrón existente |
| Parser (Expresión de ejecución) | **Media** | Agregar nuevo tipo de expresión con parsing de path |
| Parser (Statement CLI args) | **Baja** | Reconocer `><identifier` como statement |
| AST (Nodo ExecuteExpr) | **Baja** | Un struct simple |
| AST (Nodo CliArgsCaptureStmt) | **Baja** | Un struct simple |
| Interpreter (Ejecución) | **Baja** | Reutilizar `load_module()` existente |
| Interpreter (Captura output) | **Muy Baja** | Ya está implementado con `with_output()` |
| Interpreter (CLI args capture) | **Baja** | Asignar array de strings a variable |

**Tiempo Estimado**: 3-5 horas de desarrollo + testing

---

## 🎯 Recomendaciones Finales

### ✅ FACTIBLE - Implementación Completa:

1. **Ejecución Transaccional**: `</ path.zy />`
   - ✅ Factible
   - ✅ Sin conflictos
   - ✅ Infraestructura existente reutilizable

2. **Parámetros CLI**: Operador simbólico `><variable`
   - ✅ Factible
   - ✅ Sin conflictos sintácticos
   - ✅ Mantiene filosofía language-agnostic
   - ✅ Usuario elige nombre de variable en su idioma
   - ✅ Coherente con otros operadores de Zymbol

### 🎨 Filosofía Mantenida:

- **Simbolismo puro**: `><` en lugar de palabras como `ARGS`, `argv`, etc.
- **Flexibilidad lingüística**: Usuario nombra en español, inglés, coreano, griego, hebreo, etc.
- **Coherencia**: Sigue patrón de operadores direccionales (`>>`, `<<`, `<#`, `<~`, `><`)

---

## 📝 Sintaxis Final Propuesta

### Ejecución de Archivos:

```zymbol
// Ejecución directa (imprime en pantalla)
</ app_test.zy />

// Captura de output
resultado = </ app_test.zy />
>> "El script retornó: " resultado ¶
```

### Acceso a Argumentos CLI:

```zymbol
// Capturar argumentos (usuario elige el nombre)
><parametros

// Usar como variable normal
>> "Número de argumentos: " parametros$# ¶

@ arg:parametros {
    >> "Argumento: " arg ¶
}

// Verificar argumentos específicos
? parametros$# > 0 && parametros[0] == "--version" {
    >> "Zymbol-Lang v1.0.0" ¶
}
```

### Ejemplo Completo (Multi-idioma):

```zymbol
# main

// Capturar CLI args
><args

// Verificar si hay argumentos
? args$# == 0 {
    >> "Uso: zymbol main.zy <comando>" ¶
    >> "Comandos: test, build, run" ¶
}
_{
    comando = args[0]

    ?? comando {
        "test"  : { </ ./tests/run_tests.zy /> }
        "build" : { </ ./build/compile.zy /> }
        "run"   : {
            salida = </ ./app/main_app.zy />
            >> "Resultado: " salida ¶
        }
        _       : { >> "Comando desconocido: " comando ¶ }
    }
}
```

### Ejemplo en Otros Idiomas:

```zymbol
// Español
><parametros
? parametros$# > 0 { >> parametros[0] ¶ }

// Coreano
><파라미터
? 파라미터$# > 0 { >> 파라미터[0] ¶ }

// Griego
><παράμετροι
? παράμετροι$# > 0 { >> παράμετροι[0] ¶ }

// Hebreo
><פרמטרים
? פרמטרים$# > 0 { >> פרמטרים[0] ¶ }
```

---

## ✅ CONCLUSIÓN

**FACTIBILIDAD**: **ALTA** ✅

La implementación es completamente factible con:
- Infraestructura existente (captura de output, carga de archivos)
- Mínimos cambios requeridos (3 tokens nuevos, 2 nodos AST)
- **Solución simbólica adoptada**: `><variable` mantiene filosofía language-agnostic de Zymbol

**Sintaxis Final Aprobada**:
- Ejecución: `</ path.zy />` y `var = </ path.zy />`
- CLI args: `><nombre_elegido_por_usuario`

**Próximos Pasos**:
1. ✅ Análisis de factibilidad completo
2. Implementar tokens `</`, `/>`, y `><` en el lexer
3. Agregar AST nodes: `ExecuteExpr` y `CliArgsCaptureStmt`
4. Implementar parser para ambas construcciones
5. Implementar evaluación en interpreter:
   - Ejecución de archivos (print vs capture)
   - Captura de CLI args en variable especificada
6. Pasar argumentos CLI al intérprete desde main.rs
7. Testing completo con ejemplos multi-idioma

---

**Autor**: Claude Code
**Revisado**: Análisis de código fuente completo
**Estado**: Listo para implementación con aprobación de usuario

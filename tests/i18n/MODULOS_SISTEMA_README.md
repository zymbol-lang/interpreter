# Módulos del Sistema en Zymbol

Este documento describe los módulos construidos sobre la funcionalidad `<\ \>` (ejecución de comandos bash) en Zymbol.

## Filosofía de Diseño

Los módulos del sistema en Zymbol demuestran el **minimalismo extremo**: en lugar de implementar cada funcionalidad nativamente en el intérprete, delegamos al sistema operativo usando comandos bash. Esto proporciona:

1. **Implementación mínima**: No necesitas modificar el intérprete para cada nueva funcionalidad
2. **Flexibilidad total**: Acceso a todas las herramientas del sistema
3. **Portabilidad Unix**: Funciona en cualquier sistema con bash
4. **Extensibilidad**: Los usuarios pueden crear sus propias bibliotecas fácilmente

## Regla Importante: Usar Strings Entrecomillados

**IMPORTANTE**: Cuando uses comandos bash con `<\ \>`, SIEMPRE entrecomilla:
- Nombres de archivos con puntos: `"archivo.txt"`, `"calculator.zy"`
- Flags y opciones: `"-lh"`, `"-rf"`, `"-n"`
- Rutas con caracteres especiales: `"prueba_dir/archivo.txt"`
- Formatos de fecha: `"+%Y-%m-%d"`

### Ejemplos Correctos vs Incorrectos

```zymbol
// ✅ CORRECTO - con strings
<\ cat "archivo.txt" \>
<\ ls "-la" \>
<\ date "+%Y-%m-%d" \>
<\ rm "-rf" "temp_dir" \>

// ❌ INCORRECTO - sin strings (el parser separa los tokens)
<\ cat archivo.txt \>      // Se parsea como: cat, archivo, ., txt
<\ ls -la \>                // Se parsea como: ls, -, la
<\ date +%Y-%m-%d \>       // Se parsea como: date, +, %, Y, -, %, m, -, %, d
```

## Módulos Disponibles

### 1. `fecha_hora.zy` - Manejo de Fechas y Horas

Ubicación: `matematicas/fecha_hora.zy`

**Funciones:**
- `fecha_actual()` → Fecha en formato YYYY-MM-DD
- `hora_actual()` → Hora en formato HH:MM:SS
- `timestamp()` → Unix timestamp (segundos desde epoch)
- `dia_semana()` → Día de la semana (Monday, Tuesday, ...)
- `mes()` → Mes actual (January, February, ...)
- `año()` → Año actual (YYYY)
- `formato_iso()` → Fecha en formato ISO 8601
- `fecha_hora_completa()` → Fecha y hora legible completa

**Ejemplo de uso:**
```zymbol
<# ./matematicas/fecha_hora <= fecha

>> "Hoy es: " fecha::fecha_actual() ¶
>> "Son las: " fecha::hora_actual() ¶
>> "Timestamp: " fecha::timestamp() ¶
>> "Día: " fecha::dia_semana() ¶
```

### 2. `sistema.zy` - Operaciones del Sistema

Ubicación: `matematicas/sistema.zy`

**Funciones:**
- `listar()` → Lista archivos del directorio actual
- `listar_detalle()` → Lista con detalles (ls -lh)
- `listar_ocultos()` → Incluye archivos ocultos (ls -la)
- `directorio_actual()` → Retorna el directorio actual (pwd)
- `usuario_actual()` → Usuario actual (whoami)
- `nombre_host()` → Nombre del host (hostname)
- `contar_archivos()` → Cuenta archivos en el directorio
- `espacio_disco()` → Muestra espacio en disco disponible
- `procesos()` → Cuenta procesos activos del sistema

**Ejemplo de uso:**
```zymbol
<# ./matematicas/sistema <= sys

>> "Usuario: " sys::usuario_actual() ¶
>> "Host: " sys::nombre_host() ¶
>> "Ubicación: " sys::directorio_actual() ¶
>> "Archivos: " sys::contar_archivos() ¶

// Generar reporte del sistema
>> "=== Reporte del Sistema ===" ¶
>> "Directorio: " sys::directorio_actual() ¶
>> "Usuario: " sys::usuario_actual() ¶
archivos = sys::listar()
>> archivos ¶
```

### 3. `archivos.zy` - Manipulación de Archivos y Carpetas

Ubicación: `matematicas/archivos.zy`

**Funciones:**
- `leer_readme()` → Lee el contenido de README.md
- `crear_archivo_vacio()` → Crea un archivo vacío de ejemplo
- `crear_directorio()` → Crea un directorio de ejemplo
- `eliminar_temporal()` → Elimina archivo temporal
- `existe_archivo()` → Verifica si calculator.zy existe
- `existe_directorio()` → Verifica si matematicas/ existe
- `buscar_zy()` → Busca todos los archivos .zy
- `info_archivo()` → Información de calculator.zy
- `estructura_directorios()` → Muestra estructura de directorios
- `archivos_recientes()` → Lista archivos modificados recientemente

**Ejemplo de uso:**
```zymbol
<# ./matematicas/archivos <= arch

// Consultar existencia
>> "¿Existe calculator.zy? " arch::existe_archivo() ¶
>> "¿Existe matematicas/? " arch::existe_directorio() ¶

// Buscar archivos
busqueda = arch::buscar_zy()
>> "Archivos .zy encontrados:" ¶
>> busqueda ¶

// Ver estructura
estructura = arch::estructura_directorios()
>> "Estructura de directorios:" ¶
>> estructura ¶
```

## Uso Avanzado: Comandos Bash Directos

Además de usar los módulos, puedes ejecutar comandos bash directamente para casos específicos:

### Leer Archivos

```zymbol
// Leer archivo completo
contenido = <\ cat "mi_archivo.txt" \>
>> contenido ¶

// Primeras 5 líneas
inicio = <\ head "-n" 5 "archivo.txt" \>
>> inicio ¶

// Últimas 3 líneas
final = <\ tail "-n" 3 "archivo.txt" \>
>> final ¶
```

### Crear Archivos con Contenido

```zymbol
// Crear archivo con contenido (sobrescribe)
<\ echo "Hola Mundo" > "saludo.txt" \>

// Agregar contenido al final (append)
<\ echo "Segunda linea" >> "saludo.txt" \>

// Verificar
contenido = <\ cat "saludo.txt" \>
>> contenido ¶
```

### Operaciones con Directorios

```zymbol
// Crear directorio
<\ mkdir "-p" "mi_directorio" \>

// Crear estructura de directorios
<\ mkdir "-p" "proyecto/src/modules" \>

// Listar contenido
listado = <\ ls "mi_directorio" \>
>> listado ¶

// Eliminar directorio vacío
<\ rmdir "mi_directorio" \>

// Eliminar directorio con contenido
<\ rm "-rf" "mi_directorio" \>
```

### Copiar y Mover Archivos

```zymbol
// Copiar archivo
<\ cp "origen.txt" "destino.txt" \>

// Copiar con subdirectorios
<\ cp "-r" "carpeta_origen" "carpeta_destino" \>

// Mover/renombrar
<\ mv "viejo_nombre.txt" "nuevo_nombre.txt" \>

// Mover a directorio
<\ mv "archivo.txt" "carpeta/" \>
```

### Buscar Archivos

```zymbol
// Buscar por nombre
resultados = <\ find . "-name" "*.zy" \>
>> resultados ¶

// Buscar archivos grandes (más de 1MB)
grandes = <\ find . "-size" "+1M" \>
>> grandes ¶

// Buscar modificados hoy
recientes = <\ find . "-mtime" 0 \>
>> recientes ¶
```

### Información de Archivos

```zymbol
// Tamaño de archivo
tamaño = <\ du "-h" "archivo.txt" \>
>> tamaño ¶

// Permisos
permisos = <\ ls "-l" "archivo.txt" \>
>> permisos ¶

// Tipo de archivo
tipo = <\ file "archivo.txt" \>
>> tipo ¶
```

## Ejemplo Completo: Script de Respaldo

```zymbol
# script_respaldo

// Script que crea respaldo de archivos importantes

<# ./matematicas/fecha_hora <= fecha
<# ./matematicas/sistema <= sys

>> "=== Script de Respaldo ===" ¶
¶

// Obtener fecha para el nombre del respaldo
fecha_hoy = fecha::fecha_actual()
>> "Fecha: " fecha_hoy ¶

// Crear directorio de respaldo
nombre_backup = "backup"
<\ mkdir "-p" \> nombre_backup \>
>> "Directorio de respaldo creado" ¶

// Copiar archivos importantes
<\ cp "*.zy" "backup/" \>
>> "Archivos .zy copiados" ¶

// Contar archivos respaldados
cantidad = <\ ls "backup" | wc "-l" \>
>> "Total de archivos respaldados: " cantidad ¶

// Crear reporte
<\ echo "Respaldo realizado" > "backup/REPORTE.txt" \>
<\ echo fecha_hoy >> "backup/REPORTE.txt" \>

>> "Respaldo completado exitosamente" ¶
```

## Notas de Implementación

### Limitación Actual

Actualmente, los comandos bash dentro de `<\ \>` son **estáticos** - no puedes interpolar variables de Zymbol directamente. Por ejemplo:

```zymbol
// ❌ NO FUNCIONA (aún)
archivo = "mi_archivo.txt"
contenido = <\ cat archivo \>  // No interpola la variable

// ✅ SOLUCIÓN ACTUAL
// Usar valores hardcodeados o crear scripts separados
contenido = <\ cat "mi_archivo.txt" \>
```

### Futuras Mejoras Posibles

1. **Interpolación de variables**: Permitir `<\ cat {archivo} \>` para usar variables de Zymbol
2. **Parámetros en funciones**: Pasar rutas dinámicas a funciones del módulo
3. **Manejo de errores**: Capturar códigos de salida y stderr por separado

Por ahora, la solución es crear funciones específicas para casos comunes o usar valores hardcodeados.

## Conclusión

Los módulos del sistema demuestran el poder del diseño minimalista de Zymbol: con una sola característica (`<\ \>`), podemos construir una biblioteca estándar completa que rivaliza con lenguajes mucho más complejos, delegando el trabajo pesado al sistema operativo.

**Ventajas de este enfoque:**
- ✅ Menos código en el intérprete
- ✅ Más flexible y poderoso
- ✅ Fácil de extender
- ✅ Aprovecha herramientas existentes del SO
- ✅ Portable en sistemas Unix/Linux

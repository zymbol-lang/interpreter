# Integraciones Avanzadas en Zymbol

Este documento demuestra cómo Zymbol puede interactuar con bases de datos, consumir APIs REST, y realizar operaciones complejas usando solo la funcionalidad `<\ \>` con interpolación de variables `{variable}`.

## Filosofía: "Shell as a Platform"

Zymbol usa el shell como plataforma de integración universal. En lugar de implementar bibliotecas nativas para cada servicio, delegamos a herramientas CLI existentes:

- **APIs REST** → `curl`, `wget`
- **Bases de datos** → `sqlite3`, `mysql`, `psql`, `mongodb`
- **Archivos JSON** → `jq`
- **Archivos XML** → `xmllint`
- **CSV** → `awk`, `cut`, `sed`
- **Compresión** → `tar`, `gzip`, `zip`
- **Git** → `git`
- **Docker** → `docker`
- **Kubernetes** → `kubectl`

## 1. Consumo de APIs REST con curl

### Módulo HTTP (`http.zy`)

```zymbol
# http

#> {
    get
    post
    get_json
    post_json
}

// GET simple
get(url) {
    resultado = <\ curl "-s" {url} \>
    <~ resultado
}

// POST con datos
post(url, data) {
    resultado = <\ curl "-s" "-X" "POST" "-d" {data} {url} \>
    <~ resultado
}

// GET esperando JSON
get_json(url) {
    resultado = <\ curl "-s" "-L" "-H" "Accept: application/json" {url} \>
    <~ resultado
}

// POST con JSON
post_json(url, json_data) {
    resultado = <\ curl "-s" "-X" "POST" "-H" "Content-Type: application/json" "-d" {json_data} {url} \>
    <~ resultado
}
```

### Ejemplo de Uso: Consumir API Pública

```zymbol
<# ./matematicas/http <= http

// Obtener datos de API
url = "https://api.github.com/users/octocat"
usuario = http::get_json(url)
>> "Usuario de GitHub:" ¶
>> usuario ¶

// Consultar múltiples endpoints
base_url = "https://httpbin.org"

uuid_url = base_url + "/uuid"
uuid_data = http::get(uuid_url)
>> "UUID generado: " uuid_data ¶

ip_url = base_url + "/ip"
ip_data = http::get(ip_url)
>> "Mi IP: " ip_data ¶

// POST a API
webhook_url = "https://webhook.site/your-unique-url"
payload = "{\"mensaje\": \"Hola desde Zymbol\", \"timestamp\": 1735508400}"
respuesta = http::post_json(webhook_url, payload)
>> "Respuesta: " respuesta ¶
```

### Casos de Uso Reales

#### 1. Monitoreo de Servicios

```zymbol
<# ./matematicas/http <= http

// URLs a monitorear
urls = [
    "https://ejemplo.com/api/health",
    "https://otro-servicio.com/status",
    "https://backend.app.com/ping"
]

// Por ahora manual (en futuro con loops)
url1 = "https://httpbin.org/status/200"
status1 = <\ curl "-s" "-o" "/dev/null" "-w" "%{http_code}" {url1} \>
>> "Servicio 1: " status1 ¶

url2 = "https://httpbin.org/status/404"
status2 = <\ curl "-s" "-o" "/dev/null" "-w" "%{http_code}" {url2} \>
>> "Servicio 2: " status2 ¶
```

#### 2. Integración con Webhooks

```zymbol
// Enviar notificación a Slack, Discord, etc.
webhook_url = "https://hooks.slack.com/services/YOUR/WEBHOOK/URL"
mensaje = "{\"text\": \"Deploy completado exitosamente\"}"

notificar = <\ curl "-s" "-X" "POST" "-H" "Content-Type: application/json" "-d" {mensaje} {webhook_url} \>
>> "Notificación enviada" ¶
```

#### 3. Consultar APIs de Clima

```zymbol
// Usando API pública de clima
ciudad = "Madrid"
api_key = "tu_api_key_aqui"
url = "https://api.openweathermap.org/data/2.5/weather?q=" + ciudad + "&appid=" + api_key

clima = <\ curl "-s" {url} \>
>> "Clima en " ciudad ":" ¶
>> clima ¶
```

## 2. Bases de Datos

### SQLite

```zymbol
# db_sqlite

#> {
    crear_tabla
    insertar
    consultar
    actualizar
    eliminar
    query
}

// Crear tabla
crear_tabla(db, nombre_tabla) {
    query = "CREATE TABLE IF NOT EXISTS " + nombre_tabla + " (id INTEGER PRIMARY KEY, nombre TEXT, valor TEXT);"
    <\ sqlite3 {db} {query} \>
    <~ "Tabla creada"
}

// Insertar datos
insertar(db, tabla, nombre, valor) {
    query = "INSERT INTO " + tabla + " (nombre, valor) VALUES ('" + nombre + "', '" + valor + "');"
    <\ sqlite3 {db} {query} \>
    <~ "Registro insertado"
}

// Consultar datos
consultar(db, tabla) {
    query = "SELECT * FROM " + tabla + ";"
    resultado = <\ sqlite3 {db} {query} \>
    <~ resultado
}

// Ejecutar query personalizado
query(db, sql) {
    resultado = <\ sqlite3 {db} {sql} \>
    <~ resultado
}
```

### Ejemplo de Uso

```zymbol
<# ./matematicas/db_sqlite <= db

database = "mi_app.db"
tabla = "configuracion"

// Crear tabla
db::crear_tabla(database, tabla)

// Insertar configuración
db::insertar(database, tabla, "app_name", "Zymbol App")
db::insertar(database, tabla, "version", "1.0.0")
db::insertar(database, tabla, "debug", "#1")

// Consultar todo
configs = db::consultar(database, tabla)
>> "Configuraciones:" ¶
>> configs ¶

// Query personalizado
sql = "SELECT nombre, valor FROM " + tabla + " WHERE nombre LIKE '%app%';"
resultado = db::query(database, sql)
>> "Configuraciones de app:" ¶
>> resultado ¶
```

### PostgreSQL

```zymbol
// Conexión a PostgreSQL usando psql
host = "localhost"
port = "5432"
user = "postgres"
dbname = "mi_base"

// Ejecutar query
query = "SELECT id, nombre, email FROM usuarios LIMIT 10;"
usuarios = <\ psql "-h" {host} "-p" {port} "-U" {user} "-d" {dbname} "-t" "-c" {query} \>
>> usuarios ¶

// Con variable de ambiente para password
<\ export PGPASSWORD=mi_password && psql "-h" {host} "-U" {user} "-d" {dbname} "-c" {query} \>
```

### MySQL

```zymbol
// Conexión a MySQL
host = "localhost"
user = "root"
database = "mi_app"

query = "SELECT * FROM productos WHERE precio > 100;"
productos = <\ mysql "-h" {host} "-u" {user} "-p" {database} "-e" {query} \>
>> productos ¶
```

## 3. Procesamiento de JSON con jq

```zymbol
// Obtener JSON de API
url = "https://api.github.com/users/octocat"
json_raw = <\ curl "-s" {url} \>

// Extraer campos específicos con jq
nombre = <\ echo {json_raw} | jq "-r" ".name" \>
>> "Nombre: " nombre ¶

repos = <\ echo {json_raw} | jq "-r" ".public_repos" \>
>> "Repositorios públicos: " repos ¶

// Transformar JSON
json_transformado = <\ echo {json_raw} | jq "{usuario: .login, repos: .public_repos}" \>
>> "Datos transformados: " json_transformado ¶
```

## 4. Operaciones con Git

```zymbol
// Estado del repositorio
status = <\ git status "--short" \>
>> "Estado de Git:" ¶
>> status ¶

// Obtener último commit
ultimo_commit = <\ git log "-1" "--oneline" \>
>> "Último commit: " ultimo_commit ¶

// Obtener branch actual
branch = <\ git rev-parse "--abbrev-ref" "HEAD" \>
>> "Branch actual: " branch ¶

// Crear tag con variable
version = "v1.2.3"
mensaje = "Release version " + version
<\ git tag "-a" {version} "-m" {mensaje} \>
>> "Tag creado: " version ¶

// Push a remote con variable
remote = "origin"
<\ git push {remote} {branch} \>
```

## 5. Docker y Contenedores

```zymbol
// Listar contenedores
contenedores = <\ docker ps "--format" "{{.Names}}" \>
>> "Contenedores activos:" ¶
>> contenedores ¶

// Iniciar contenedor con variable
container_name = "mi_app"
<\ docker start {container_name} \>
>> "Contenedor iniciado: " container_name ¶

// Ver logs
logs = <\ docker logs "--tail" "50" {container_name} \>
>> "Últimos 50 logs:" ¶
>> logs ¶

// Ejecutar comando en contenedor
comando = "ls -la /app"
output = <\ docker exec {container_name} {comando} \>
>> output ¶

// Build con variables
image_name = "mi-app:v1.0"
dockerfile = "./Dockerfile"
<\ docker build "-t" {image_name} "-f" {dockerfile} "." \>
```

## 6. Procesamiento de CSV

```zymbol
// Leer CSV y procesar
archivo_csv = "datos.csv"

// Obtener encabezados
headers = <\ head "-n" 1 {archivo_csv} \>
>> "Columnas: " headers ¶

// Contar filas
total_filas = <\ wc "-l" {archivo_csv} \>
>> "Total de filas: " total_filas ¶

// Extraer columna específica (columna 2)
columna = 2
datos_col = <\ cut "-d" "," "-f" {columna} {archivo_csv} \>
>> "Datos de columna " columna ":" ¶
>> datos_col ¶

// Filtrar filas que contengan texto
buscar = "activo"
filtrados = <\ grep {buscar} {archivo_csv} \>
>> "Registros con '" buscar "':" ¶
>> filtrados ¶
```

## 7. Automatización de Tareas

### Backup Automatizado

```zymbol
<# ./matematicas/fecha_hora <= fecha
<# ./matematicas/sistema <= sys

// Obtener fecha para nombre de backup
fecha_actual = fecha::fecha_actual()
hora_actual = fecha::hora_actual()
timestamp = fecha_actual + "_" + hora_actual

// Directorios
origen = "/app/data"
destino_base = "/backups"
nombre_backup = "backup_" + timestamp + ".tar.gz"
ruta_completa = destino_base + "/" + nombre_backup

// Crear backup
>> "Creando backup..." ¶
<\ tar "-czf" {ruta_completa} {origen} \>
>> "Backup creado: " nombre_backup ¶

// Verificar tamaño
tamaño = <\ du "-h" {ruta_completa} | cut "-f1" \>
>> "Tamaño: " tamaño ¶

// Subir a servidor remoto (ejemplo)
servidor = "backup@servidor.com"
ruta_remota = "/var/backups/"
<\ scp {ruta_completa} {servidor}:{ruta_remota} \>
>> "Backup subido al servidor remoto" ¶
```

### Monitor de Salud del Sistema

```zymbol
<# ./matematicas/sistema <= sys
<# ./matematicas/fecha_hora <= fecha

>> "=== Reporte de Salud del Sistema ===" ¶
>> "Generado: " fecha::fecha_hora_completa() ¶
¶

// CPU
cpu = <\ top "-bn1" | grep "Cpu(s)" \>
>> "CPU: " cpu ¶

// Memoria
memoria = <\ free "-h" | grep "Mem:" \>
>> "Memoria: " memoria ¶

// Disco
disco = <\ df "-h" "/" | tail "-1" \>
>> "Disco: " disco ¶

// Procesos
procesos = sys::procesos()
>> "Total de procesos: " procesos ¶

// Uptime
uptime = <\ uptime \>
>> "Uptime: " uptime ¶
```

## 8. Integración Completa: Pipeline de Datos

```zymbol
// Pipeline: API → Procesamiento → Base de Datos → Notificación

<# ./matematicas/http <= http
<# ./matematicas/db_sqlite <= db

>> "=== Pipeline de Datos ===" ¶

// 1. Obtener datos de API
api_url = "https://api.example.com/data"
datos_json = http::get_json(api_url)
>> "Paso 1: Datos obtenidos de API" ¶

// 2. Procesar con jq (extraer campos relevantes)
datos_procesados = <\ echo {datos_json} | jq ".results[]" \>
>> "Paso 2: Datos procesados" ¶

// 3. Guardar en base de datos
database = "pipeline.db"
db::crear_tabla(database, "datos_api")
// Insertar datos (simplificado)
db::query(database, "INSERT INTO datos_api (raw_data) VALUES ('" + datos_procesados + "');")
>> "Paso 3: Datos guardados en BD" ¶

// 4. Enviar notificación
webhook = "https://hooks.slack.com/your-webhook"
mensaje = "{\"text\": \"Pipeline completado exitosamente\"}"
http::post_json(webhook, mensaje)
>> "Paso 4: Notificación enviada" ¶

>> "Pipeline completado" ¶
```

## Ventajas de este Enfoque

1. **Minimalismo**: No necesitas bibliotecas nativas para cada servicio
2. **Flexibilidad**: Acceso a TODO el ecosistema de herramientas Unix
3. **Portabilidad**: Funciona en cualquier sistema con bash
4. **Rapidez de desarrollo**: No esperas a que el lenguaje implemente features
5. **Poder**: Acceso completo a herramientas maduras y probadas

## Limitaciones Actuales

1. **Sin sanitización**: Los queries SQL necesitan escapado manual
2. **Manejo de errores básico**: No hay try/catch aún
3. **Parsing de JSON manual**: Se requiere `jq` externo
4. **Sin conexiones persistentes**: Cada llamada crea nueva conexión

## Futuras Mejoras

1. Sanitización automática de SQL
2. Parser JSON nativo (o wrapper seguro de jq)
3. Manejo de errores estructurado
4. Pool de conexiones para bases de datos
5. Async/await para operaciones HTTP

## Conclusión

Con solo `<\ \>` y la interpolación de variables `{variable}`, Zymbol puede:
- ✅ Consumir cualquier API REST
- ✅ Interactuar con bases de datos (SQLite, MySQL, PostgreSQL, MongoDB)
- ✅ Procesar JSON, XML, CSV
- ✅ Automatizar Git, Docker, Kubernetes
- ✅ Crear pipelines de datos completos
- ✅ Monitorear sistemas
- ✅ Generar backups automatizados

**Todo esto sin agregar ni una sola línea de código nativo al intérprete.** Esto es el poder del diseño minimalista: delega al ecosistema existente en lugar de reimplementar todo.

# SQLite Integration with Zymbol

This document shows how Zymbol successfully integrates with SQLite using only bash command execution (`<\ \>`) and variable interpolation (`{variable}`).

## ✅ Installation Verified

SQLite 3.46.1 installed successfully on Debian Linux:
```bash
sudo apt install sqlite3
sqlite3 --version  # 3.46.1
```

## ✅ Working Features

### 1. Simple Database Operations
Test file: `test_sqlite_simple.zy`

- ✅ Create database and table
- ✅ Insert data
- ✅ Query data
- ✅ Cleanup

**Example output:**
```
=== SQLite Simple Test ===

1. Creating database, table, and inserting data...
   Database ready

2. Querying data:
1|Hello from Zymbol!
2|SQLite works!

3. Cleanup - removing test database
   Database removed

=== Test Complete ===
```

### 2. Full CRUD Operations
Test file: `test_database.zy`
Module: `matematicas/db.zy`

- ✅ Create database
- ✅ Create tables
- ✅ Insert records (multiple)
- ✅ Query all records
- ✅ Query by ID
- ✅ Update records
- ✅ Custom queries (COUNT, WHERE clauses)
- ✅ Delete records
- ✅ Cleanup

**Example output:**
```
=== Test de Base de Datos SQLite ===

1. Creando base de datos: test_zymbol.db
   Base de datos creada

2. Creando tabla usuarios...
   Tabla creada

3. Insertando usuarios...
   3 usuarios insertados

4. Consultando todos los usuarios:
1|Juan Perez|juan@example.com|30
2|Juan Perez|juan@example.com|30
3|Juan Perez|juan@example.com|30

7. Consultas personalizadas:
   Total de usuarios: 3

   Usuarios mayores de 25:
Juan Perez|31
Juan Perez|30
Juan Perez|30

=== Fin del Test de Base de Datos ===
```

## Key Learnings

### 1. Variable Interpolation Works
```zymbol
database = "test.db"
result = <\ sqlite3 {database} "SELECT * FROM users;" \>
```

The `{database}` variable is correctly interpolated into the bash command.

### 2. SQL Must Be Quoted
When passing SQL to sqlite3, it **must** be a quoted string literal in the bash command:

✅ **Correct:**
```zymbol
result = <\ sqlite3 {db} "SELECT * FROM users;" \>
```

❌ **Incorrect:**
```zymbol
query = "SELECT * FROM users;"
result = <\ sqlite3 {db} {query} \>  // Fails - SQL not quoted!
```

The variable interpolation inserts the raw value, not a quoted string. The shell then interprets `SELECT`, `*`, `FROM`, etc. as separate tokens, causing syntax errors.

### 3. Database Persistence
Each `sqlite3` command is a **separate shell session**, but the database **file persists** on disk:
- ✅ CREATE TABLE in one command → Table saved to file
- ✅ INSERT in another command → Data added to same file
- ✅ SELECT in another command → Reads from same file

This allows sequential operations across multiple Zymbol statements.

### 4. Combining Operations
For operations that need to happen atomically, combine SQL statements with semicolons:
```zymbol
dummy = <\ sqlite3 {db} "CREATE TABLE test (id INTEGER); INSERT INTO test VALUES (1); INSERT INTO test VALUES (2);" \>
```

## Module Design: db.zy

The database module provides reusable functions:

```zymbol
# db

#> {
    crear_db
    crear_tabla_usuarios
    insertar_usuario
    consultar_usuarios
    consultar_usuario_por_id
    actualizar_usuario
    eliminar_usuario
}

// Create database
crear_db(nombre_db) {
    resultado = <\ sqlite3 {nombre_db} "SELECT 1;" \>
    <~ resultado
}

// Create users table
crear_tabla_usuarios(db) {
    resultado = <\ sqlite3 {db} "CREATE TABLE IF NOT EXISTS usuarios (id INTEGER PRIMARY KEY, nombre TEXT, email TEXT, edad INTEGER);" \>
    <~ resultado
}

// Insert user
insertar_usuario(db) {
    resultado = <\ sqlite3 {db} "INSERT INTO usuarios (nombre, email, edad) VALUES ('Juan Perez', 'juan@example.com', 30);" \>
    <~ resultado
}

// Query all users
consultar_usuarios(db) {
    resultado = <\ sqlite3 {db} "SELECT * FROM usuarios;" \>
    <~ resultado
}
```

**Design principle:** SQL is embedded directly in the bash command as a string literal, not passed through a variable.

## Running the Tests

```bash
cd /home/rakzo/github/mini-project/tests/i18n

# Simple test
cargo run --quiet -- run test_sqlite_simple.zy

# Direct test
cargo run --quiet -- run test_sqlite_direct.zy

# Full CRUD test with module
cargo run --quiet -- run test_database.zy
```

## Next Steps

Potential improvements:
1. **SQL Injection Protection**: Add sanitization for user input
2. **Dynamic Queries**: Build SQL strings safely from variables
3. **Parameterized Queries**: Use sqlite3 parameter binding
4. **Error Handling**: Check for SQL errors and handle gracefully
5. **Transactions**: Support BEGIN/COMMIT/ROLLBACK

## Conclusion

Zymbol successfully demonstrates the **"Shell as a Platform"** philosophy:
- ✅ No native SQLite bindings needed
- ✅ Full database functionality through bash
- ✅ Variable interpolation for dynamic commands
- ✅ Modular, reusable code
- ✅ Simple, clean syntax

**"Delegate to the ecosystem instead of reimplementing everything."**

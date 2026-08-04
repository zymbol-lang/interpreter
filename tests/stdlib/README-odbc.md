# Setting up the ODBC data source the `std/db` tests use

`stdlib_db_basic.zy`, `stdlib_db_tx.zy` and `stdlib_i18n_db_es.zy` connect through a
data source named **`zymbol_sqlite`**, backed by SQLite. Without it they fail at
`db::connect`; every other suite runs regardless.

## Why a DSN and not a driver name

The tests used to say `Driver={SQLite3}`, which is what unixODBC calls the driver
after `apt install libsqliteodbc`. The Windows installer for the same driver
registers it as `SQLite3 ODBC Driver`, so the connection string could only ever work
on one of the two platforms — and it was the platform the tests were written on.

A DSN is ODBC's answer to precisely that: the driver's name stays in each machine's
own configuration, and the test names a data source. One line of setup per platform
buys the same test running everywhere.

There is a second reason on Windows. Braces are string interpolation in Zymbol, so a
connection string containing them has to be written `Driver=\{...\}`. That
`Driver={SQLite3}` parsed at all was luck: `SQLite3` happens to be a valid
identifier, and Zymbol leaves an interpolation of an unknown name alone.
`SQLite3 ODBC Driver` has spaces and is a lex error. A DSN sidesteps the whole
question.

## Linux

```sh
sudo apt install libsqliteodbc     # or the distro equivalent

cat >> ~/.odbc.ini <<'EOF'

[zymbol_sqlite]
Driver = SQLite3
Database =
EOF
```

`Driver` here is the name from `/etc/odbcinst.ini`; check it with `odbcinst -q -d`.
`Database` is left empty on purpose — each test supplies its own file in the
connection string, so they cannot collide.

## macOS

```sh
brew install sqliteodbc unixodbc

cat >> ~/.odbc.ini <<'EOF'

[zymbol_sqlite]
Driver = /opt/homebrew/lib/libsqlite3odbc.dylib
Database =
EOF
```

Homebrew does not always register the driver under a name, so pointing `Driver` at
the dylib is the reliable form. Adjust the path for an Intel Mac
(`/usr/local/lib/...`).

## Windows

Install the driver from <http://www.ch-werner.de/sqliteodbc/> — `sqliteodbc_w64.exe`
for a 64-bit build of Zymbol. That step needs administrator rights, because
registering an ODBC *driver* writes to `HKLM`.

Creating the data source does not: a user DSN lives in `HKCU`, so this runs as
yourself.

```powershell
$dsn  = 'zymbol_sqlite'
$base = 'HKCU:\SOFTWARE\ODBC\ODBC.INI'
$dll  = (Get-ItemProperty 'HKLM:\SOFTWARE\ODBC\ODBCINST.INI\SQLite3 ODBC Driver').Driver

New-Item -Path "$base\$dsn" -Force | Out-Null
New-ItemProperty -Path "$base\$dsn" -Name Driver   -Value $dll -PropertyType String -Force | Out-Null
New-ItemProperty -Path "$base\$dsn" -Name Database -Value ''   -PropertyType String -Force | Out-Null

New-Item -Path "$base\ODBC Data Sources" -Force | Out-Null
New-ItemProperty -Path "$base\ODBC Data Sources" -Name $dsn `
                 -Value 'SQLite3 ODBC Driver' -PropertyType String -Force | Out-Null
```

Match the bitness: a 64-bit `zymbol.exe` needs the 64-bit driver and a DSN under
`HKCU\SOFTWARE\ODBC`, while a 32-bit build would want
`HKCU\SOFTWARE\WOW6432Node\ODBC`. Mixing them produces
`architecture mismatch between the Driver and Application`, which reads like a
missing driver but is not.

To remove it again:

```powershell
Remove-Item 'HKCU:\SOFTWARE\ODBC\ODBC.INI\zymbol_sqlite' -Recurse
Remove-ItemProperty 'HKCU:\SOFTWARE\ODBC\ODBC.INI\ODBC Data Sources' -Name zymbol_sqlite
```

## Checking it

```sh
zymbol run tests/stdlib/stdlib_db_basic.zy
```

Expected:

```
2
1
O'Brien & Co.
#1
#0
```

The `.db` files land in the working directory, not in `/tmp` — Windows has none —
and are covered by `.gitignore`.

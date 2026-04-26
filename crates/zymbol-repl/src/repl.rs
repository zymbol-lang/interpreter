//! Main REPL implementation

use crate::colors;
use crate::line_editor::LineEditor;
use crate::raw_writer::RawModeWriter;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::Stylize,
    terminal::{self, ClearType},
};
use std::io::{self, Write};
use zymbol_interpreter::{Interpreter, Value};

/// The REPL instance
pub struct Repl {
    /// The persistent interpreter instance; uses RawModeWriter so that
    /// bare `\n` from the interpreter becomes `\r\n` in raw-mode terminal.
    interpreter: Interpreter<RawModeWriter<io::Stdout>>,
    /// The line editor for input handling
    editor: LineEditor,
    /// Whether the REPL should continue running
    running: bool,
}

impl Default for Repl {
    fn default() -> Self {
        Self::new()
    }
}

impl Repl {
    /// Create a new REPL instance
    pub fn new() -> Self {
        let mut interpreter = Interpreter::with_output(RawModeWriter::new(io::stdout()));

        // When a `<<` input statement executes, the terminal must temporarily leave raw
        // mode so the user can type with echo and press Enter normally.
        interpreter.set_input_fn(|| {
            terminal::disable_raw_mode()?;
            let mut buf = String::new();
            io::stdin().read_line(&mut buf)?;
            terminal::enable_raw_mode()?;
            Ok(buf)
        });

        Self {
            interpreter,
            editor: LineEditor::new(),
            running: true,
        }
    }

    /// Start the REPL loop
    pub fn start(&mut self) -> io::Result<()> {
        // Enable raw mode for terminal
        terminal::enable_raw_mode()?;

        // Print welcome message
        self.print_welcome()?;

        // Main REPL loop
        while self.running {
            // Print prompt and get input
            self.print_prompt()?;

            // Read and process input
            match self.read_line() {
                Ok(Some(line)) => {
                    // Add to history before processing
                    self.editor.add_to_history(line.clone());

                    // Process the input
                    self.process_input(&line)?;
                }
                Ok(None) => {
                    // User pressed Esc or Ctrl+C without selection
                    continue;
                }
                Err(e) => {
                    // Restore terminal before propagating error
                    terminal::disable_raw_mode()?;
                    return Err(e);
                }
            }
        }

        // Restore terminal
        terminal::disable_raw_mode()?;

        Ok(())
    }

    /// Print the welcome message
    fn print_welcome(&self) -> io::Result<()> {
        let mut stdout = io::stdout();
        execute!(
            stdout,
            terminal::Clear(ClearType::All),
            cursor::MoveTo(0, 0)
        )?;
        writeln!(stdout, "Zymbol-Lang REPL v0.0.1")?;
        writeln!(stdout, "Type HELP for commands, EXIT to quit\r")?;
        writeln!(stdout)?;
        stdout.flush()
    }

    /// Print the prompt
    fn print_prompt(&self) -> io::Result<()> {
        let mut stdout = io::stdout();
        write!(stdout, "\r{}", colors::prompt())?;
        stdout.flush()
    }

    /// Read a line of input with the line editor
    fn read_line(&mut self) -> io::Result<Option<String>> {
        let mut stdout = io::stdout();

        loop {
            // Render current state
            self.render_line(&mut stdout)?;

            // Read event
            if let Event::Key(key_event) = event::read()? {
                match self.handle_key_event(key_event) {
                    KeyAction::Continue => continue,
                    KeyAction::Submit => {
                        // Move to next line
                        writeln!(stdout, "\r")?;
                        stdout.flush()?;
                        return Ok(Some(self.editor.submit()));
                    }
                    KeyAction::Cancel => {
                        self.editor.clear();
                        writeln!(stdout, "\r")?;
                        stdout.flush()?;
                        return Ok(None);
                    }
                    KeyAction::Exit => {
                        self.running = false;
                        writeln!(stdout, "\r")?;
                        stdout.flush()?;
                        return Ok(None);
                    }
                    KeyAction::ClearScreen => {
                        execute!(stdout, terminal::Clear(ClearType::All), cursor::MoveTo(0, 0))?;
                        continue;
                    }
                }
            }
        }
    }

    /// Render the current line with cursor
    fn render_line(&self, stdout: &mut io::Stdout) -> io::Result<()> {
        // Clear current line and reprint
        write!(stdout, "\r{}", colors::prompt())?;

        let buffer = self.editor.buffer();
        let cursor_pos = self.editor.cursor_pos();

        // Handle selection highlighting
        if let Some((start, end)) = self.editor.selection() {
            // Print text with selection highlighted
            write!(stdout, "{}", &buffer[..start])?;
            write!(
                stdout,
                "{}",
                crossterm::style::style(&buffer[start..end])
                    .on(crossterm::style::Color::Blue)
            )?;
            write!(stdout, "{}", &buffer[end..])?;
        } else {
            write!(stdout, "{}", buffer)?;
        }

        // Clear to end of line
        execute!(stdout, terminal::Clear(ClearType::UntilNewLine))?;

        // Position cursor
        let cursor_col = colors::prompt_visible_length() + count_display_width(&buffer[..cursor_pos]);
        execute!(stdout, cursor::MoveToColumn(cursor_col as u16))?;

        stdout.flush()
    }

    /// Handle a key event
    fn handle_key_event(&mut self, event: KeyEvent) -> KeyAction {
        match (event.code, event.modifiers) {
            // Submit on Enter
            (KeyCode::Enter, _) => KeyAction::Submit,

            // Cancel on Escape
            (KeyCode::Esc, _) => KeyAction::Cancel,

            // Exit on Ctrl+C (if no selection) or Ctrl+D
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                if self.editor.has_selection() {
                    self.editor.copy_selection();
                    KeyAction::Continue
                } else {
                    KeyAction::Exit
                }
            }
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => KeyAction::Exit,

            // Clear screen on Ctrl+L
            (KeyCode::Char('l'), KeyModifiers::CONTROL) => KeyAction::ClearScreen,

            // Cut on Ctrl+X
            (KeyCode::Char('x'), KeyModifiers::CONTROL) => {
                self.editor.cut_selection();
                KeyAction::Continue
            }

            // Paste on Ctrl+V
            (KeyCode::Char('v'), KeyModifiers::CONTROL) => {
                self.editor.paste();
                KeyAction::Continue
            }

            // Cursor movement
            (KeyCode::Left, KeyModifiers::NONE) => {
                self.editor.cursor_left();
                KeyAction::Continue
            }
            (KeyCode::Right, KeyModifiers::NONE) => {
                self.editor.cursor_right();
                KeyAction::Continue
            }
            (KeyCode::Home, KeyModifiers::NONE) => {
                self.editor.cursor_home();
                KeyAction::Continue
            }
            (KeyCode::End, KeyModifiers::NONE) => {
                self.editor.cursor_end();
                KeyAction::Continue
            }

            // Selection with Shift+Arrow
            (KeyCode::Left, KeyModifiers::SHIFT) => {
                self.editor.select_left();
                KeyAction::Continue
            }
            (KeyCode::Right, KeyModifiers::SHIFT) => {
                self.editor.select_right();
                KeyAction::Continue
            }
            (KeyCode::Home, KeyModifiers::SHIFT) => {
                self.editor.select_home();
                KeyAction::Continue
            }
            (KeyCode::End, KeyModifiers::SHIFT) => {
                self.editor.select_end();
                KeyAction::Continue
            }

            // History navigation
            (KeyCode::Up, KeyModifiers::NONE) => {
                self.editor.history_up();
                KeyAction::Continue
            }
            (KeyCode::Down, KeyModifiers::NONE) => {
                self.editor.history_down();
                KeyAction::Continue
            }

            // Backspace and Delete
            (KeyCode::Backspace, _) => {
                self.editor.backspace();
                KeyAction::Continue
            }
            (KeyCode::Delete, _) => {
                self.editor.delete();
                KeyAction::Continue
            }

            // Regular character input
            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                self.editor.insert_char(c);
                KeyAction::Continue
            }

            // Tab (insert spaces)
            (KeyCode::Tab, _) => {
                self.editor.insert_str("    ");
                KeyAction::Continue
            }

            _ => KeyAction::Continue,
        }
    }

    /// Process user input
    fn process_input(&mut self, input: &str) -> io::Result<()> {
        let trimmed = input.trim();
        let mut stdout = io::stdout();

        // Handle empty input
        if trimmed.is_empty() {
            return Ok(());
        }

        // Handle commands (case-insensitive)
        match trimmed.to_uppercase().as_str() {
            "HELP" => self.show_help(&mut stdout),
            "EXIT" | "QUIT" => {
                self.running = false;
                writeln!(stdout, "Goodbye!\r")?;
                Ok(())
            }
            "VARS" => self.show_variables(&mut stdout),
            "CLEAR" => {
                execute!(stdout, terminal::Clear(ClearType::All), cursor::MoveTo(0, 0))?;
                Ok(())
            }
            "HISTORY" => self.show_history(&mut stdout),
            _ => {
                // Check for variable inspection (name?)
                if trimmed.ends_with('?') && trimmed.len() > 1 {
                    let var_name = &trimmed[..trimmed.len() - 1];
                    self.inspect_variable(var_name, &mut stdout)
                } else {
                    // Execute as Zymbol code
                    self.execute_code(input, &mut stdout)
                }
            }
        }
    }

    /// Show help information
    fn show_help(&self, stdout: &mut io::Stdout) -> io::Result<()> {
        writeln!(stdout, "\r")?;
        writeln!(stdout, "{}", colors::command("Commands:"))?;
        writeln!(stdout, "  {}     - Show this help\r", colors::command("HELP"))?;
        writeln!(stdout, "  {}     - Exit the REPL\r", colors::command("EXIT"))?;
        writeln!(stdout, "  {}     - List all defined variables\r", colors::command("VARS"))?;
        writeln!(stdout, "  {}    - Clear the screen\r", colors::command("CLEAR"))?;
        writeln!(stdout, "  {}  - Show command history\r", colors::command("HISTORY"))?;
        writeln!(stdout, "\r")?;
        writeln!(stdout, "{}", colors::command("Variable Inspection:"))?;
        writeln!(stdout, "  {}   - Show type and value of variable\r", colors::type_name("name?"))?;
        writeln!(stdout, "\r")?;
        writeln!(stdout, "{}", colors::command("Keyboard Shortcuts:"))?;
        writeln!(stdout, "  Enter       - Execute current line\r")?;
        writeln!(stdout, "  Esc         - Cancel current input\r")?;
        writeln!(stdout, "  Ctrl+C      - Exit (or copy if selection)\r")?;
        writeln!(stdout, "  Ctrl+L      - Clear screen\r")?;
        writeln!(stdout, "  Up/Down     - Navigate history\r")?;
        writeln!(stdout, "  Shift+Arrow - Select text\r")?;
        writeln!(stdout, "  Ctrl+X      - Cut selection\r")?;
        writeln!(stdout, "  Ctrl+V      - Paste\r")?;
        writeln!(stdout, "\r")?;
        stdout.flush()
    }

    /// Show all defined variables
    fn show_variables(&self, stdout: &mut io::Stdout) -> io::Result<()> {
        let variables = self.interpreter.list_variables();

        if variables.is_empty() {
            writeln!(stdout, "No variables defined\r")?;
        } else {
            writeln!(stdout, "\r")?;
            for (name, value) in variables {
                let type_name = value_type_name(&value);
                writeln!(
                    stdout,
                    "  {}: {} = {}\r",
                    name,
                    colors::type_name(&type_name),
                    self.interpreter.format_value(&value)
                )?;
            }
            writeln!(stdout, "\r")?;
        }
        stdout.flush()
    }

    /// Show command history
    fn show_history(&self, stdout: &mut io::Stdout) -> io::Result<()> {
        let history = self.editor.get_history();

        if history.is_empty() {
            writeln!(stdout, "No history\r")?;
        } else {
            writeln!(stdout, "\r")?;
            for (i, cmd) in history.iter().enumerate() {
                writeln!(stdout, "  {}: {}\r", i + 1, cmd)?;
            }
            writeln!(stdout, "\r")?;
        }
        stdout.flush()
    }

    /// Inspect a variable
    fn inspect_variable(&self, name: &str, stdout: &mut io::Stdout) -> io::Result<()> {
        match self.interpreter.get_variable_info(name) {
            Some((type_name, value)) => {
                writeln!(
                    stdout,
                    "{}: {} = {}\r",
                    name,
                    colors::type_name(&type_name),
                    colors::value(&self.interpreter.format_value(&value))
                )?;
            }
            None => {
                writeln!(stdout, "{}\r", colors::error(&format!("Variable '{}' not found", name)))?;
            }
        }
        stdout.flush()
    }

    /// Execute Zymbol code
    fn execute_code(&mut self, code: &str, stdout: &mut io::Stdout) -> io::Result<()> {
        // Reset newline tracking so we can detect dangling output from this execution.
        self.interpreter.writer_mut().reset_newline_tracking();

        let result = self.interpreter.execute_line(code);

        // Flush interpreter output before inspecting the newline state.
        self.interpreter.flush_output()?;

        // If the interpreter produced output without a trailing newline (e.g. `>> x`
        // without `¶`), move to a fresh line before showing the next prompt or result.
        if !self.interpreter.writer().ended_with_newline() {
            stdout.write_all(b"\r\n")?;
        }

        match result {
            Ok(Some(value)) => {
                if !matches!(value, Value::Unit) {
                    let repr = self.interpreter.format_value_repr(&value);
                    let type_label = value_type_name(&value);
                    writeln!(
                        stdout,
                        "{} {} {}{}{}\r",
                        colors::result_arrow(),
                        colors::value(&repr),
                        colors::dim("::"),
                        colors::dim(" "),
                        colors::type_name(&type_label),
                    )?;
                }
            }
            Ok(None) => {}
            Err(e) => {
                writeln!(stdout, "{}\r", colors::error(&format!("Error: {}", e)))?;
            }
        }
        stdout.flush()
    }
}

/// Action to take after handling a key event
enum KeyAction {
    /// Continue reading input
    Continue,
    /// Submit the current line
    Submit,
    /// Cancel the current input
    Cancel,
    /// Exit the REPL
    Exit,
    /// Clear the screen
    ClearScreen,
}

/// Get the type name for a value using Zymbol's symbolic notation
/// ###=Int, ##.=Float, ##"=String, ##'=Char, ##?=Bool, ##]=Array, ##)=Tuple, ##_=Unit
fn value_type_name(value: &Value) -> String {
    match value {
        Value::Int(_) => "###".to_string(),
        Value::Float(_) => "##.".to_string(),
        Value::String(_) => "##\"".to_string(),
        Value::Char(_) => "##'".to_string(),
        Value::Bool(_) => "##?".to_string(),
        Value::Array(elements) => {
            if elements.is_empty() {
                "##]".to_string()
            } else {
                format!("##]<{}>", value_type_name(&elements[0]))
            }
        }
        Value::Tuple(elements) => {
            let types: Vec<String> = elements.iter().map(value_type_name).collect();
            format!("##)({})", types.join(", "))
        }
        Value::NamedTuple(fields) => {
            let types: Vec<String> = fields
                .iter()
                .map(|(name, val)| format!("{}: {}", name, value_type_name(val)))
                .collect();
            format!("##)({})", types.join(", "))
        }
        Value::Function(_) => "##->".to_string(),
        Value::Error(err) => format!("##{}", err.error_type),
        Value::Unit => "##_".to_string(),
    }
}

/// Count display width of a string (accounting for wide characters)
fn count_display_width(s: &str) -> usize {
    s.chars().count()
}

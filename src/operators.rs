use std::collections::HashMap;
use std::fs::{File, read_to_string};
use std::hash::Hash;
use std::path::Path;

use rand::Rng;

use crate::context::{Context, Port};
use crate::midi::MidiNote;
use crate::operators::Update::Variables;

pub fn char_to_base_36(c: char) -> (u8, bool) {
    if c >= '0' && c <= '9' {
        (c as u8 - '0' as u8, false)
    } else if c >= 'a' && c <= 'z' {
        (c as u8 + 10 - 'a' as u8, false)
    } else if c >= 'A' && c <= 'Z' {
        (c as u8 + 10 - 'A' as u8, true)
    } else {
        (0, false)
    }
}

pub fn base_36_to_char(c: u8, upper: bool) -> char {
    let c = c % 36;
    let c = if c < 10 {
        c as u8 + '0' as u8
    } else if upper {
        c as u8 - 10 + 'A' as u8
    } else {
        c as u8 - 10 + 'a' as u8
    };
    c as char
}

enum Update {
    Inputs(Vec<Port>),
    Outputs(Vec<Port>),
    Locks(Vec<Port>),
    Notes(Vec<MidiNote>),
    Variables(Vec<(char, char)>),
}

#[derive(Clone)]
pub struct Operator {
    name: String,
    evaluate: fn(context: &Context, row: i32, col: i32) -> Vec<Update>,
}


impl Operator {
    fn new(name: &str, evaluate: fn(&Context, i32, i32) -> Vec<Update>) -> Operator {
        Operator { name: String::from(name), evaluate }
    }

    fn apply(&self, context: &mut Context, row: i32, col: i32) {
        if !context.is_locked(row, col) {
            let updates = (self.evaluate)(context, row, col);
            for update in updates {
                match update {
                    Update::Inputs(ports) => {
                        for port in ports {
                            context.lock(port.row, port.col);
                        }
                    }
                    Update::Outputs(ports) => {
                        for port in ports {
                            context.write(port.row, port.col, port.value);
                            context.lock(port.row, port.col);
                        }
                    }
                    Update::Locks(ports) => {
                        for port in ports {
                            context.lock(port.row, port.col);
                        }
                    }
                    Update::Notes(notes) => {
                        for note in notes {
                            context.write_note(note);
                        }
                    }
                    Update::Variables(variables) => {
                        for (name, value) in variables {
                            context.set_variable(name, value);
                        }
                    }
                }
            }
        }
    }
}

pub fn read_operator_config(filename: &str) -> HashMap<String, char> {
    let default_operator_config = "
A Add
B Sub
C Clock
D Delay
E East
F If
G Generate
H Halt
I Increment
J Jump
K Concat
L Lesser
M Multiply
N North
O Read
P Push
Q Query
R Random
S South
T Track
U Euclid
V Variable
W West
X Write
Y Jymp
Z Interpolate
# Comment
: Midi
".trim().to_string();
    read_to_string(filename)
        .unwrap_or(default_operator_config)
        .lines()
        .filter_map(|line| line.split_once(' '))
        .filter_map(|(symbol, name)| {
            if let Some(symbol) = symbol.chars().next() {
                Some((name.to_string(), symbol))
            } else {
                None
            }
        }).collect()
}

pub fn get_tick_operators(operator_map: &HashMap<String, char>) -> HashMap<char, Operator> {
    vec![
        Operator::new("Add", add),
        Operator::new("Sub", sub),
        Operator::new("Clock", clock),
        Operator::new("Delay", delay),
        Operator::new("East", east),
        Operator::new("If", condition),
        Operator::new("Generate", generate),
        Operator::new("Halt", halt),
        Operator::new("Increment", increment),
        Operator::new("Jump", jump),
        Operator::new("Concat", concat),
        Operator::new("Lesser", lesser),
        Operator::new("Multiply", multiply),
        Operator::new("North", north),
        Operator::new("Read", read),
        Operator::new("Push", push),
        Operator::new("Query", query),
        Operator::new("Random", random),
        Operator::new("South", south),
        Operator::new("Track", track),
        Operator::new("Euclid", euclid),
        Operator::new("Variable", variable),
        Operator::new("West", west),
        Operator::new("Write", write),
        Operator::new("Jymp", jymp),
        Operator::new("Interpolate", interpolate),
        Operator::new("Comment", comment),
        // the midi operator is technically operated each tick, but only produces a note on a bang
        Operator::new("Midi", midi_note),
    ].iter().cloned().filter_map(
        |operator| {
            if let Some(&symbol) = operator_map.get(&operator.name) {
                Some((symbol, operator))
            } else {
                None
            }
        }
    ).collect()
}

fn add(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let a_port = context.listen("a", row, col - 1, '0');
    let b_port = context.listen("b", row, col + 1, '0');

    let (a, a_upper) = char_to_base_36(a_port.value);
    let (b, b_upper) = char_to_base_36(b_port.value);
    let out = base_36_to_char(a + b, a_upper || b_upper);

    let out_port = Port::new("out", row + 1, col, out);

    vec![
        Update::Inputs(vec![a_port, b_port]),
        Update::Outputs(vec![out_port]),
    ]
}

fn sub(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let a_port = context.listen("a", row, col - 1, '0');
    let b_port = context.listen("b", row, col + 1, '0');

    let (a, a_upper) = char_to_base_36(a_port.value);
    let (b, b_upper) = char_to_base_36(b_port.value);
    let diff = if a > b { a - b } else { b - a };
    let out = base_36_to_char(diff, a_upper || b_upper);

    let out_port = Port::new("out", row + 1, col, out);

    vec![
        Update::Inputs(vec![a_port, b_port]),
        Update::Outputs(vec![out_port]),
    ]
}

fn delay(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let rate_port = context.listen("rate", row, col - 1, '1');
    let mod_port = context.listen("mod", row, col + 1, '8');

    let (rate, _) = char_to_base_36(rate_port.value);
    let (delay_mod, _) = char_to_base_36(mod_port.value);
    let rate = rate.max(1);
    let delay_mod = delay_mod.max(1);

    let mut out_port = context.listen("out", row + 1, col, '\0');
    if context.ticks % (rate as usize * delay_mod as usize) == 0 {
        out_port.value = '*';
    }

    vec![
        Update::Inputs(vec![rate_port, mod_port]),
        Update::Outputs(vec![out_port]),
    ]
}

fn random(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let min_port = context.listen("min", row, col - 1, '0');
    let max_port = context.listen("max", row, col + 1, 'z');

    let (min, min_upper) = char_to_base_36(min_port.value);
    let (max, max_upper) = char_to_base_36(max_port.value);
    let max = max.max(min + 1); // wow this looks like trash

    let mut rng = rand::thread_rng();
    let r = rng.gen_range(min..max);
    let out = base_36_to_char(r, min_upper || max_upper);

    let out_port = Port::new("out", row + 1, col, out);

    vec![
        Update::Inputs(vec![min_port, max_port]),
        Update::Outputs(vec![out_port]),
    ]
}

fn midi_note(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let channel_port = context.listen("channel", row, col + 1, '0');
    let octave_port = context.listen("octave", row, col + 2, '0');
    let note_port = context.listen("note", row, col + 3, '0');
    let velocity_port = context.listen("velocity", row, col + 4, 'f');
    let duration_port = context.listen("duration", row, col + 5, '1');

    let (channel, _) = char_to_base_36(channel_port.value);
    let (octave, _) = char_to_base_36(octave_port.value);
    let (note, note_upper) = char_to_base_36(note_port.value);
    let (velocity, _) = char_to_base_36(velocity_port.value);
    let (duration, _) = char_to_base_36(duration_port.value);

    let midi_notes = if note >= 10 && (
        context.read(row - 1, col) == '*'
            || context.read(row, col - 1) == '*'
            || context.read(row + 1, col) == '*'
    ) {
        vec![MidiNote::from_base_36(
            channel, octave, note, !note_upper,
            velocity, duration, context.tick_time,
        )]
    } else {
        vec![]
    };

    vec![
        Update::Inputs(vec![channel_port, octave_port, note_port, velocity_port, duration_port]),
        Update::Notes(midi_notes),
    ]
}

fn clock(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let rate_port = context.listen("rate", row, col - 1, '1');
    let mod_port = context.listen("mod", row, col + 1, '8');

    let (rate, _) = char_to_base_36(rate_port.value);
    let (clock_mod, mod_upper) = char_to_base_36(mod_port.value);
    let rate = rate.max(1);
    let clock_mod = clock_mod.max(1);
    let out = context.ticks / rate as usize % clock_mod as usize;
    let out = base_36_to_char(out as u8, mod_upper);

    let out_port = Port::new("out", row + 1, col, out);

    vec![
        Update::Inputs(vec![rate_port, mod_port]),
        Update::Outputs(vec![out_port]),
    ]
}

fn track(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let key_port = context.listen("key", row, col - 2, '0');
    let len_port = context.listen("len", row, col - 1, '1');

    let (key, _) = char_to_base_36(key_port.value);
    let (len, _) = char_to_base_36(len_port.value);
    let len = len.max(1);
    let val_port = context.listen("val", row, col + 1 + (key % len) as i32, '\0');
    let out = val_port.value;

    let out_port = Port::new("out", row + 1, col, out);
    let locks = (0..(len as i32)).map(
        |i| Port::new("locked", row, col + 1 + i, '\0')
    ).collect();

    vec![
        Update::Inputs(vec![key_port, len_port, val_port]),
        Update::Outputs(vec![out_port]),
        Update::Locks(locks)
    ]
}

fn halt(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let output_port = context.listen("out", row + 1, col, '\0');
    vec![
        Update::Inputs(vec![output_port.clone()]),
        Update::Outputs(vec![output_port.clone()]),
        Update::Locks(vec![output_port]),
    ]
}

fn east(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let mut input_port = context.listen("", row, col, '\0');
    let mut output_port = context.listen("", row, col + 1, '\0');
    if output_port.value == '\0' {
        output_port.value = input_port.value;
        input_port.value = '\0';
        vec![
            Update::Outputs(vec![input_port, output_port.clone()]),
            Update::Locks(vec![output_port]),
        ]
    } else {
        input_port.value = '*';
        vec![
            Update::Outputs(vec![input_port])
        ]
    }
}

fn west(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let mut input_port = context.listen("", row, col, '\0');
    let mut output_port = context.listen("", row, col - 1, '\0');
    if output_port.value == '\0' {
        output_port.value = input_port.value;
        input_port.value = '\0';
        vec![
            Update::Outputs(vec![input_port, output_port.clone()]),
            Update::Locks(vec![output_port]),
        ]
    } else {
        input_port.value = '*';
        vec![
            Update::Outputs(vec![input_port])
        ]
    }
}

fn north(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let mut input_port = context.listen("", row, col, '\0');
    let mut output_port = context.listen("", row - 1, col, '\0');
    if output_port.value == '\0' {
        output_port.value = input_port.value;
        input_port.value = '\0';
        vec![
            Update::Outputs(vec![input_port, output_port.clone()]),
            Update::Locks(vec![output_port]),
        ]
    } else {
        input_port.value = '*';
        vec![
            Update::Outputs(vec![input_port])
        ]
    }
}

fn south(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let mut input_port = context.listen("", row, col, '\0');
    let mut output_port = context.listen("", row + 1, col, '\0');
    if output_port.value == '\0' {
        output_port.value = input_port.value;
        input_port.value = '\0';
        vec![
            Update::Outputs(vec![input_port, output_port.clone()]),
            Update::Locks(vec![output_port]),
        ]
    } else {
        input_port.value = '*';
        vec![
            Update::Outputs(vec![input_port])
        ]
    }
}

fn condition(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let a_port = context.listen("a", row, col - 1, '\0');
    let b_port = context.listen("b", row, col + 1, '\0');

    let (a, _) = char_to_base_36(a_port.value);
    let (b, _) = char_to_base_36(b_port.value);
    let mut out_port = context.listen("out", row + 1, col, '\0');
    if a == b {
        out_port.value = '*';
    }

    vec![
        Update::Inputs(vec![a_port, b_port]),
        Update::Outputs(vec![out_port]),
    ]
}

fn increment(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let step_port = context.listen("step", row, col - 1, '1');
    let mod_port = context.listen("mod", row, col + 1, 'z');

    let (step, _) = char_to_base_36(step_port.value);
    let (increment_mod, mod_upper) = char_to_base_36(mod_port.value);
    let increment_mod = increment_mod.max(1);
    let mut out_port = context.listen("out", row + 1, col, '0');
    let (out, _) = char_to_base_36(out_port.value);
    let out = (out + step) % increment_mod;
    out_port.value = base_36_to_char(out, mod_upper);

    vec![
        Update::Inputs(vec![step_port, mod_port]),
        Update::Outputs(vec![out_port]),
    ]
}

fn jump(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let input_port = context.listen("input", row - 1, col, '\0');
    let output_port = Port::new("output", row + 1, col, input_port.value);

    vec![
        Update::Inputs(vec![input_port]),
        Update::Outputs(vec![output_port]),
    ]
}

fn jymp(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let input_port = context.listen("input", row, col - 1, '\0');
    let output_port = Port::new("output", row, col + 1, input_port.value);

    vec![
        Update::Inputs(vec![input_port]),
        Update::Outputs(vec![output_port]),
    ]
}

fn lesser(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let a_port = context.listen("a", row, col - 1, '\0');
    let b_port = context.listen("b", row, col + 1, '\0');

    let out = if a_port.value != '\0' && b_port.value != '\0' {
        let (a, a_upper) = char_to_base_36(a_port.value);
        let (b, b_upper) = char_to_base_36(b_port.value);
        let less = if a < b { a } else { b };
        base_36_to_char(less, a_upper || b_upper)
    } else {
        '\0'
    };

    let out_port = Port::new("out", row + 1, col, out);

    vec![
        Update::Inputs(vec![a_port, b_port]),
        Update::Outputs(vec![out_port]),
    ]
}

fn multiply(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let a_port = context.listen("a", row, col - 1, '0');
    let b_port = context.listen("b", row, col + 1, '0');

    let (a, a_upper) = char_to_base_36(a_port.value);
    let (b, b_upper) = char_to_base_36(b_port.value);
    let out = base_36_to_char(a.saturating_mul(b), a_upper || b_upper);

    let out_port = Port::new("out", row + 1, col, out);

    vec![
        Update::Inputs(vec![a_port, b_port]),
        Update::Outputs(vec![out_port]),
    ]
}

fn read(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let x_port = context.listen("x", row, col - 2, '0');
    let y_port = context.listen("y", row, col - 1, '0');

    let (x, _) = char_to_base_36(x_port.value);
    let (y, _) = char_to_base_36(y_port.value);
    let val_port = context.listen("val", row + y as i32, col + 1 + x as i32, '\0');
    let out = val_port.value;

    let out_port = Port::new("out", row + 1, col, out);

    vec![
        Update::Inputs(vec![x_port, y_port, val_port]),
        Update::Outputs(vec![out_port]),
    ]
}

fn push(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let key_port = context.listen("key", row, col - 2, '0');
    let len_port = context.listen("len", row, col - 1, '1');

    let (key, _) = char_to_base_36(key_port.value);
    let (len, _) = char_to_base_36(len_port.value);
    let len = len.max(1);
    let val_port = context.listen("val", row, col + 1, '\0');
    let out = val_port.value;

    let out_port = Port::new("out", row + 1, col + (key % len) as i32, out);
    let locks = (0..(len as i32)).map(
        |i| Port::new("locked", row + 1, col + i, '\0')
    ).collect();

    vec![
        Update::Inputs(vec![key_port, len_port, val_port]),
        Update::Outputs(vec![out_port]),
        Update::Locks(locks)
    ]
}

fn query(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let x_port = context.listen("x", row, col - 3, '0');
    let y_port = context.listen("y", row, col - 2, '0');
    let len_port = context.listen("len", row, col - 1, '1');

    let (x, _) = char_to_base_36(x_port.value);
    let (y, _) = char_to_base_36(y_port.value);
    let (len, _) = char_to_base_36(len_port.value);
    let len = len.max(1);
    let mut input_ports: Vec<Port> = (0..len).map(|i| context.listen(
        &format!("in-{}", i), row + y as i32, col + 1 + x as i32 + i as i32, '\0',
    )).collect();
    let output_ports = input_ports.iter().enumerate().map(|(i, port)| Port::new(
        &format!("out-{}", i), row + 1, col + 1 + i as i32 - len as i32, port.value,
    )).collect();

    input_ports.extend(vec![x_port, y_port]);
    vec![
        Update::Inputs(input_ports),
        Update::Outputs(output_ports),
    ]
}

fn generate(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let x_port = context.listen("x", row, col - 3, '0');
    let y_port = context.listen("y", row, col - 2, '0');
    let len_port = context.listen("len", row, col - 1, '1');

    let (x, _) = char_to_base_36(x_port.value);
    let (y, _) = char_to_base_36(y_port.value);
    let (len, _) = char_to_base_36(len_port.value);
    let len = len.max(1);
    let mut input_ports: Vec<Port> = (0..len).map(|i| context.listen(
        &format!("in-{}", i), row, col + 1 + i as i32, '\0',
    )).collect();
    let output_ports = input_ports.iter().enumerate().map(|(i, port)| Port::new(
        &format!("out-{}", i), row + 1 + y as i32, col + i as i32 + x as i32, port.value,
    )).collect();

    input_ports.extend(vec![x_port, y_port]);
    vec![
        Update::Inputs(input_ports),
        Update::Outputs(output_ports),
    ]
}

fn write(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let x_port = context.listen("x", row, col - 2, '0');
    let y_port = context.listen("y", row, col - 1, '0');

    let (x, _) = char_to_base_36(x_port.value);
    let (y, _) = char_to_base_36(y_port.value);
    let val_port = context.listen("val", row, col + 1, '\0');
    let out = val_port.value;

    let out_port = Port::new("out", row + 1 + y as i32, col + x as i32, out);

    vec![
        Update::Inputs(vec![x_port, y_port, val_port]),
        Update::Outputs(vec![out_port]),
    ]
}

fn interpolate(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let rate_port = context.listen("rate", row, col - 1, '1');
    let target_port = context.listen("target", row, col + 1, 'z');

    let (rate, _) = char_to_base_36(rate_port.value);
    let (target, target_upper) = char_to_base_36(target_port.value);
    let mut out_port = context.listen("out", row + 1, col, '0');
    let (out, _) = char_to_base_36(out_port.value);
    let out = (out + rate).min(target);
    out_port.value = base_36_to_char(out, target_upper);

    vec![
        Update::Inputs(vec![rate_port, target_port]),
        Update::Outputs(vec![out_port]),
    ]
}

fn euclid(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let step_port = context.listen("step", row, col - 1, '1');
    let max_port = context.listen("max", row, col + 1, '8');

    let (step, _) = char_to_base_36(step_port.value);
    let (max, _) = char_to_base_36(max_port.value);
    let max = max.max(1);

    let mut out_port = context.listen("out", row + 1, col, '\0');
    if (step as usize * (context.ticks + max as usize - 1) % max as usize) as u8 + step >= max {
        out_port.value = '*';
    }

    vec![
        Update::Inputs(vec![step_port, max_port]),
        Update::Outputs(vec![out_port]),
    ]
}

fn comment(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let width = context.width as i32;
    let mut c = col + 1;
    for i in c..width {
        c = i;
        if context.read(row, c) == '#' {
            break;
        }
    }
    let locks = (col..(c + 1)).map(|l| Port::new("locked", row, l, '\0')).collect();
    vec![
        Update::Locks(locks)
    ]
}

fn variable(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let write_port = context.listen("write", row, col - 1, '\0');
    let read_port = context.listen("read", row, col + 1, '\0');

    if write_port.value == '\0' {
        let out_port = Port::new("out", row + 1, col, context.read_variable(read_port.value));
        vec![
            Update::Inputs(vec![write_port, read_port]),
            Update::Outputs(vec![out_port]),
        ]
    } else {
        let value = read_port.value;
        vec![
            Update::Inputs(vec![read_port]),
            Update::Variables(vec![(write_port.value, value)]),
        ]
    }
}

fn concat(context: &Context, row: i32, col: i32) -> Vec<Update> {
    let len_port = context.listen("len", row, col - 1, '1');

    let (len, _) = char_to_base_36(len_port.value);
    let output_ports = (0..(len as i32)).map(
        |i| Port::new(&format!("out-{}", i), row + 1, col + i + 1,
                      context.read_variable(context.read(row, col + i + 1)))
    ).collect();
    let locks = (0..(len as i32)).map(
        |i| Port::new("locked", row, col + 1 + i, '\0')
    ).collect();
    vec![
        Update::Inputs(vec![len_port]),
        Update::Outputs(output_ports),
        Update::Locks(locks),
    ]
}

pub fn get_bang_operators(operator_map: &HashMap<String, char>) -> HashMap<char, Operator> {
    let mut operators: HashMap<char, Operator> = HashMap::new();
    for (c, operator) in get_tick_operators(operator_map) {
        operators.insert(c.to_ascii_lowercase(), operator);
    }
    operators
}

pub fn grid_tick(
    context: &mut Context,
    tick_operators: &HashMap<char, Operator>,
    bang_operators: &HashMap<char, Operator>,
) {
    let rows = context.height as i32;
    let cols = context.width as i32;
    context.unlock_all();
    context.clear_all_variables();

    // clear previous bangs
    for row in 0..rows {
        for col in 0..cols {
            if context.read(row, col) == '*' {
                context.write(row, col, '\0');
            }
        }
    }

    // apply grid operators (which may produce new bangs)
    for row in 0..rows {
        for col in 0..cols {
            if let Some(operator) = tick_operators.get(&context.read(row, col)) {
                operator.apply(context, row, col);
            }
        }
    }

    // apply bang operators on current bangs
    for row in 0..rows {
        for col in 0..cols {
            if let Some(operator) = bang_operators.get(&context.read(row, col)) {
                if context.read(row - 1, col) == '*'
                    || context.read(row, col - 1) == '*'
                    || context.read(row + 1, col) == '*' {
                    operator.apply(context, row, col);
                }
            }
        }
    }

    context.ticks += 1;
}

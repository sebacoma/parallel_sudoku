use anyhow::{Context, Result};
use clap::{ArgAction, Parser};
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use crossbeam_utils::thread;
use std::fs;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

/// 9x9 Sudoku como arreglo plano de 81 u8 (0 = vacío, 1..=9 valores)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Board {
    cells: [u8; 81],
}

impl Board {
    fn from_str(s: &str) -> Result<Self> {
        let mut cells = [0u8; 81];
        let mut idx = 0usize;
        for ch in s.chars() {
            if ch.is_ascii_digit() {
                if idx >= 81 {
                    break;
                }
                cells[idx] = ch.to_digit(10).unwrap() as u8;
                idx += 1;
            }
        }
        if idx != 81 {
            anyhow::bail!("Expected 81 digits (0-9), got {}", idx);
        }
        Ok(Board { cells })
    }

    fn to_pretty(&self) -> String {
        let mut out = String::new();
        for r in 0..9 {
            if r % 3 == 0 {
                out.push_str("+-------+-------+-------+\n");
            }
            for c in 0..9 {
                if c % 3 == 0 {
                    out.push('|');
                    out.push(' ');
                }
                let v = self.cells[r * 9 + c];
                out.push(if v == 0 { '·' } else { char::from(b'0' + v) });
                out.push(' ');
            }
            out.push_str("|\n");
        }
        out.push_str("+-------+-------+-------+\n");
        out
    }

    #[inline]
    fn get(&self, r: usize, c: usize) -> u8 {
        self.cells[r * 9 + c]
    }
    #[inline]
    fn set(&mut self, r: usize, c: usize, v: u8) {
        self.cells[r * 9 + c] = v;
    }

    fn is_solved(&self) -> bool {
        self.cells.iter().all(|&x| x != 0)
    }
}

/// Bitmask de permitidos en (r,c): bits 1..=9 encendidos si se puede poner ese valor
fn allowed_mask(b: &Board, r: usize, c: usize) -> u16 {
    if b.get(r, c) != 0 {
        return 0;
    }
    let mut used = 0u16;
    // Fila
    for cc in 0..9 {
        used |= 1u16 << b.get(r, cc);
    }
    // Columna
    for rr in 0..9 {
        used |= 1u16 << b.get(rr, c);
    }
    // Subcuadrante 3x3
    let br = (r / 3) * 3;
    let bc = (c / 3) * 3;
    for rr in br..br + 3 {
        for cc in bc..bc + 3 {
            used |= 1u16 << b.get(rr, cc);
        }
    }
    // bits 1..=9 disponibles si NO están usados
    let all = 0b_11_1111_1110u16; // bits 1..9 = 1
    all & !used
}

/// Elige celda vacía con MRV (mínimo número de valores posibles)
fn choose_cell_mrv(b: &Board) -> Option<(usize, usize, u16)> {
    let mut best: Option<(usize, usize, u16, usize)> = None; // (r,c,mask,count)
    for r in 0..9 {
        for c in 0..9 {
            if b.get(r, c) == 0 {
                let m = allowed_mask(b, r, c);
                let cnt = m.count_ones() as usize;
                if cnt == 0 {
                    // contradicción (no hay valores posibles para esta celda)
                    return None;
                }
                if best.map_or(true, |(_, _, _, bcnt)| cnt < bcnt) {
                    best = Some((r, c, m, cnt));
                    if cnt == 1 {
                        break;
                    }
                }
            }
        }
    }
    best.map(|(r, c, m, _)| (r, c, m))
}

/// DFS secuencial con backtracking + MRV
fn solve_seq(mut b: Board) -> Option<Board> {
    if let Some((r, c, mask)) = choose_cell_mrv(&b) {
        let mut m = mask;
        while m != 0 {
            // extraer bit menos significativo (valor candidato)
            let bit = m & (!m + 1);
            m &= m - 1;
            let v = bit.trailing_zeros() as u8; // 1..=9
            let mut nb = b;
            nb.set(r, c, v);
            if let Some(sol) = solve_seq(nb) {
                return Some(sol);
            }
        }
        None
    } else {
        // Si no hay MRV devuelto: o está resuelto o hubo contradicción detectada arriba
        if b.is_solved() {
            Some(b)
        } else {
            None
        }
    }
}

/// Solver paralelo con k hilos y cola global.
/// Estrategia: sembrar tareas desde la primera celda MRV para dar amplitud a los workers.
fn solve_parallel_k(b: Board, k: usize) -> Option<Board> {
    if k <= 1 {
        return solve_seq(b);
    }

    // Elegir primera celda MRV para semilla
    match choose_cell_mrv(&b) {
        Some((r, c, mask)) => {
            // Generar semillas
            let mut seeds: Vec<Board> = Vec::new();
            let mut m = mask;
            while m != 0 {
                let bit = m & (!m + 1);
                m &= m - 1;
                let v = bit.trailing_zeros() as u8; // 1..=9
                let mut nb = b;
                nb.set(r, c, v);
                seeds.push(nb);
            }

            let found = Arc::new(AtomicBool::new(false));
            let (tx, rx): (Sender<Board>, Receiver<Board>) = unbounded();
            let (sol_tx, sol_rx) = bounded::<Board>(1);

            for s in seeds {
                tx.send(s).ok()?;
            }

            // Ámbito de hilos
            thread::scope(|scope| {
                for _ in 0..k {
                    let rx = rx.clone();
                    let tx = tx.clone();
                    let sol_tx = sol_tx.clone();
                    let found = found.clone();
                    scope.spawn(move |_| {
                        while !found.load(Ordering::Relaxed) {
                            // Intentar tarea rápida; si no, esperar corto
                            let task = match rx.try_recv() {
                                Ok(t) => t,
                                Err(_) => match rx.recv_timeout(Duration::from_millis(10)) {
                                    Ok(t) => t,
                                    Err(_) => break,
                                },
                            };

                            if let Some((r, c, mask)) = choose_cell_mrv(&task) {
                                let mut m = mask;
                                while m != 0 {
                                    let bit = m & (!m + 1);
                                    m &= m - 1;
                                    let v = bit.trailing_zeros() as u8;
                                    let mut nb = task;
                                    nb.set(r, c, v);

                                    // Pequeño DFS local para bajar el branching antes de reencolar
                                    if let Some(sol) = solve_seq(nb) {
                                        found.store(true, Ordering::Relaxed);
                                        let _ = sol_tx.send(sol);
                                        return;
                                    }
                                }
                            } else if task.is_solved() {
                                found.store(true, Ordering::Relaxed);
                                let _ = sol_tx.send(task);
                                return;
                            }
                            // Si hubo contradicción, seguimos con la siguiente tarea
                        }
                    });
                }
            })
                .ok()?; // si el scope falla, devolvemos None

            sol_rx.recv().ok()
        }
        None => {
            // Puede ser puzzle ya resuelto o contradicción global
            if b.is_solved() {
                Some(b)
            } else {
                None
            }
        }
    }
}

#[derive(Parser, Debug)]
#[command(version, about = "Parallel Sudoku solver in Rust (k-threads)")]
struct Cli {
    /// 81 dígitos (0=vacío) o ruta a archivo .txt que los contenga
    input: String,
    /// Número de hilos (k). Si se omite, se ejecuta benchmark k=1..=cores+1
    #[arg(short, long)]
    k: Option<usize>,
    /// Ejecuta benchmark para k = 1..=cores+1 y guarda results.csv
    #[arg(long, action = ArgAction::SetTrue)]
    bench: bool,
}

fn load_board(input: &str) -> Result<Board> {
    let content = if input.len() >= 81 && input.chars().all(|c| c.is_ascii_digit()) {
        input.to_string()
    } else {
        fs::read_to_string(input).with_context(|| format!("reading {}", input))?
    };
    Board::from_str(&content)
}

fn run_once(b: Board, k: usize) -> (Option<Board>, Duration) {
    let t0 = Instant::now();
    let sol = solve_parallel_k(b, k);
    (sol, t0.elapsed())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let b = load_board(&cli.input)?;

    if cli.bench || cli.k.is_none() {
        let cores = num_cpus::get();
        let ks: Vec<usize> = (1..=cores + 1).collect();
        println!("Benchmarking k = 1..={} on {} cores", cores + 1, cores);

        // Tiempo base con k=1
        let (sol1, t1) = run_once(b, 1);
        let sol1 = sol1.context("unsatisfiable puzzle for k=1")?;
        println!("k,seconds,speedup,efficiency");
        println!("1,{:.6},1.0,1.0", t1.as_secs_f64());

        let mut csv = String::from("k,seconds,speedup,efficiency\n");
        csv.push_str(&format!("1,{:.6},1.0,1.0\n", t1.as_secs_f64()));

        for &k in ks.iter().skip(1) {
            let (solk, tk) = run_once(b, k);
            // Sanity check: misma solución
            if solk.as_ref() != Some(&sol1) {
                eprintln!("[warn] Different solution or none for k={}", k);
            }
            let speedup = t1.as_secs_f64() / tk.as_secs_f64();
            let eff = speedup / (k as f64);
            println!("{},{:.6},{:.3},{:.3}", k, tk.as_secs_f64(), speedup, eff);
            csv.push_str(&format!(
                "{},{:.6},{:.3},{:.3}\n",
                k,
                tk.as_secs_f64(),
                speedup,
                eff
            ));
        }
        fs::write("results.csv", csv)?;
        println!("\nSaved results to results.csv");
    } else {
        let k = cli.k.unwrap();
        let (sol, dt) = run_once(b, k);
        match sol {
            Some(s) => {
                println!(
                    "Solved in {:.3} s with k={} threads\n{}",
                    dt.as_secs_f64(),
                    k,
                    s.to_pretty()
                );
            }
            None => {
                println!("No solution found (k={}, {:.3} s)", k, dt.as_secs_f64());
            }
        }
    }

    Ok(())
}

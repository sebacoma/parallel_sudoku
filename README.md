# 🧩 Solver Paralelo de Sudoku en Rust

## 📌 Descripción
Este proyecto implementa un **solucionador de Sudokus** en Rust utilizando:

- **Backtracking secuencial** con la heurística MRV (*Minimum Remaining Values*).  
- **Solver paralelo** con `k` hilos y una cola global de trabajo.  
- **Modo benchmark** para medir **speedup** y **eficiencia** variando `k` desde 1 hasta *número de núcleos + 1*.  

Trabajo desarrollado como parte de la **tarea de programación (entrega: 26 de septiembre de 2025)**.

---

## ⚙️ Requisitos
- Rust (instalado vía `rustup`)  
- Cargo (incluido con Rust)  
- macOS/Linux/Windows (probado en Apple Silicon M4 Pro)

Dependencias:
```toml
clap = { version = "4", features = ["derive"] }
num_cpus = "1"
crossbeam-channel = "0.5"
crossbeam-utils = "0.8"
anyhow = "1"
```

---

## 🚀 Uso

### 1. Compilación
```bash
cargo build --release
```

### 2. Formato de entrada
Un Sudoku debe entregarse como **81 dígitos** (0 = celda vacía).  

Ejemplo (`hard.txt`):
```
530070000600195000098000060800060003400803001700020006060000280000419005000080079
```

### 3. Ejecución
- Secuencial (k=1):
```bash
cargo run --release -- hard.txt -k 1
```

- Paralelo con 4 hilos:
```bash
cargo run --release -- hard.txt -k 4
```

- Benchmark (ejecuta con k = 1..=núcleos+1 y guarda `results.csv`):
```bash
cargo run --release -- hard.txt --bench
```

---

## 📊 Resultados del Benchmark

Salida en consola:
```
Benchmarking k = 1..=12 on 11 cores
k,seconds,speedup,efficiency
1,0.342100,1.0,1.0
2,0.181430,1.885,0.943
3,0.127900,2.674,0.891
...
```

Archivo guardado: `results.csv`  
```
k,seconds,speedup,efficiency
1,0.342100,1.0,1.0
2,0.181430,1.885,0.943
3,0.127900,2.674,0.891
...
```

---

## 📈 Métricas

- **Speedup**  
  \[
  S_k = \frac{T_1}{T_k}
  \]

- **Eficiencia**  
  \[
  E_k = \frac{S_k}{k}
  \]

Donde:
- \( T_1 \) = tiempo de ejecución con k=1 (base).  
- \( T_k \) = tiempo de ejecución con k hilos.  

---

## 🖼️ Ejemplo de Sudoku Resuelto
```
+-------+-------+-------+
| 5 3 4 | 6 7 8 | 9 1 2 |
| 6 7 2 | 1 9 5 | 3 4 8 |
| 1 9 8 | 3 4 2 | 5 6 7 |
+-------+-------+-------+
| 8 5 9 | 7 6 1 | 4 2 3 |
| 4 2 6 | 8 5 3 | 7 9 1 |
| 7 1 3 | 9 2 4 | 8 5 6 |
+-------+-------+-------+
| 9 6 1 | 5 3 7 | 2 8 4 |
| 2 8 7 | 4 1 9 | 6 3 5 |
| 3 4 5 | 2 8 6 | 1 7 9 |
+-------+-------+-------+
```

---

## 📚 Referencias
- [Efficient Parallel Sudoku Solving](https://shawnjzlee.me/dl/efficient-parallel-sudoku.pdf)  
- [Parallel Sudoku Solvers (Buffalo Univ.)](https://cse.buffalo.edu/faculty/miller/Courses/CSE633/Sankar-Spring-2014-CSE633.pdf)

---

## 👨‍💻 Autor
Sebastián Concha Macías  
Tarea — Septiembre 2025  

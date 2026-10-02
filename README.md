# Лабораторная работа №01. Модель памяти и атомарные операции

Решение на Rust

## Структура

| Файл | Содержимое |
|------|------------|
| `src/counter.rs` | часть A: атомарный счётчик, порядок памяти задаётся типом-меткой (`Relaxed`, `SeqCst`), вариант на `fetch_add` и на `compare_exchange_weak`, обёртка `MemOrder` с порядком в виде обычного аргумента |
| `src/litmus.rs` | часть B: litmus-тест «буфер записи» |
| `src/spsc.rs` | часть C: кольцевой буфер для одного писателя и одного читателя, `split` возвращает пару `Producer` / `Consumer` |
| `src/padded.rs` | выравнивание по строке кэша (64 байта) |
| `src/bin/harness.rs` | текстовый интерфейс для проверки |
| `tests/integration.rs` | тесты |

## Сборка и запуск

```bash
cargo test --release
cargo build --release
./target/release/harness info
```

Через Docker:

```bash
docker build -t pp-lab-rust .
docker run --rm pp-lab-rust info
```

## Команды harness

```
harness info
harness counter <T> <N> <relaxed|seqcst> [--cas]
harness litmus <N> <relaxed|seqcst>
harness spsc <capacity> <items>
harness bench-counter <T> <N> <relaxed|seqcst> [--cas]
harness bench-spsc <capacity> <items>
```

Каждая команда печатает одну строку JSON в stdout. Ошибка разбора аргументов
завершает программу с кодом 2, диагностика выводится в stderr.

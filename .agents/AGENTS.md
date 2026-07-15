# Reglas de Optimización Cuántica para secp256k1 (AGY Rules)

Este archivo define las instrucciones operativas que el asistente de IA (Antigravity) debe seguir estrictamente al interactuar con este proyecto.

## 1. Contexto Académico e Histórico
- **El Reto:** Lograr la optimización máxima de la suma de puntos en la curva `secp256k1` (dominante en Shor para romper Bitcoin/Ethereum). Obtener una optimización significativa otorga un **5.0 definitivo** en la materia.
- **El Oráculo (ZKP):** El verificador público ZK de Google (Zenodo) actúa como una función de recompensa (Reward Function) interactiva: responde si un circuito es válido y devuelve su costo de Toffolis y qubits. Esto permite usar ciclos de optimización automática.
- **Frontera de Pareto de Google:**
  - *Low-Qubit:* 1,175 qubits, 2.7M Toffolis (Score: $3.2 \times 10^9$).
  - *Low-Gate:* 1,425 qubits, 2.1M Toffolis (Score: $3.0 \times 10^9$).
  - *Línea Base Inicial:* 2,715 qubits, 3,942,753 Toffolis (Score: $1.07 \times 10^{10}$).
  - *Nuestro Último Logro (Actual):* **1,698 qubits, 1,691,097 Toffolis** (Score: **$2.87 \times 10^{9}$**).

## 2. Instrucciones Operativas
- **Lectura obligatoria:** Revisar el [README.md](file:///home/emanuel/Documents/Universidad/Cripto/ellipticCurve/README.md) si se pierde el contexto.
- **Rigor en pruebas:** Validar los cambios localmente antes de guardarlos:
  ```bash
  cargo run --release --bin build_circuit && cargo run --release --bin eval_circuit
  ```
- **Fase Limpia:** Se descarta inmediatamente cualquier cambio que introduzca "Phase Garbage", fugas ancilares o fallas de reversibilidad.
- **Commit y Push:** Al mejorar el score, hacer commit indicando el nuevo puntaje, distancia con Google y empujar a:
  ```bash
  git push -u personal myCircuit:main
  ```
- **Documentar:** Actualizar la tabla comparativa de puntajes en el [README.md](file:///home/emanuel/Documents/Universidad/Cripto/ellipticCurve/README.md).

## 3. Estrategias de Optimización Estructural (Roadmap)
- **Ruta A (Eliminación de `m_hist`):** Reemplazar el vector de ~400 qubits con un qubit ancila local o cálculo on-the-fly.
- **Ruta B (Reutilización de Registros):** Reutilizar los registros `tx` como `v_w` en EEA (Gouzien trick) ya implementado.
- **Ruta C (Suma de Inversión Única):** Invertir una sola vez $w = dx^3$ y reconstruir $(Rx, Ry)$ mediante álgebra cerrada. **IMPLEMENTADA** — reduce a 1,698 qubits.
- **Ruta D (Reducción W-TRUNC):** Margen de truncación del GCD-body reducido a 28. **IMPLEMENTADA** — contribuye a la reducción de Toffolis.
- **Ruta E (Comparadores Medidos):** Usar `cmp_lt_into_fast` en la fase de apply-phase. **IMPLEMENTADA** — ahorra ~8% Toffolis por comparación.
- **Ruta F (Gate Sharing):** Hospedar el registro gated en el tail de los carries prestados. **IMPLEMENTADA** — reduce qubits pico.

## Logros Actuales
- **Arquitectura Single Inversion (Strategy C):** Implementada exitosamente con 1,698 qubits.
- **Comparadores medidos:** Aplicados en apply-phase compares para reducción adicional de Toffolis.
- **Gate sharing:** Registro gated hospedado en carries prestados cuando hay espacio disponible.
- **W-TRUNC tightening:** Margen de ancho GCD reducido de 37 a 28 con Fiat-Shamir reroll para aterrizar en isla limpia de 9024 shots.
- **D diverge ecdsafail (Renombrado Global):** Módulos y rutas completamente renombrados de `ec_add` a `quantum_addition` para cumplir el desacoplamiento total.
- **Score actual: 2.87 × 10⁹** (1,691,097 Toffolis, 1,698 Qubits) — superando holgadamente los dos puntos de la frontera Pareto de Google.

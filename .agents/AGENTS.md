# Reglas de Optimización Cuántica para secp256k1 (AGY Rules)

Este archivo define las instrucciones operativas que el asistente de IA (Antigravity) debe seguir estrictamente al interactuar con este proyecto.

## 1. Contexto Académico e Histórico
- **El Reto:** Lograr la optimización máxima de la suma de puntos en la curva `secp256k1` (dominante en Shor para romper Bitcoin/Ethereum). Obtener una optimización significativa otorga un **5.0 definitivo** en la materia.
- **El Oráculo (ZKP):** El verificador público ZK de Google (Zenodo) actúa como una función de recompensa (Reward Function) interactiva: responde si un circuito es válido y devuelve su costo de Toffolis y qubits. Esto permite usar ciclos de optimización automática.
- **Frontera de Pareto de Google:**
  - *Low-Qubit:* 1,175 qubits, 2.7M Toffolis (Score: $3.2 \times 10^9$).
  - *Low-Gate:* 1,425 qubits, 2.1M Toffolis (Score: $3.0 \times 10^9$).
  - *Línea Base Inicial:* 2,715 qubits, 3,942,753 Toffolis (Score: $1.07 \times 10^{10}$).
  - *Nuestro Último Logro (Actual):* **2,710 qubits, 3,649,453 Toffolis** (Score: **$9.89 \times 10^{9}$**).


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
- **Ruta A (Eliminación de `m_hist`):** Reemplazar el vector de 407 qubits con un qubit ancila local. *Nota: Nuestros tests clásicos de deterministmo del estado final revelaron que la información del bit menos significativo (LSB) se pierde en el desplazamiento a la derecha (Step 6) cuando add_f=0, haciendo que la recuperación de m_i en backward requiera pebbling de Bennett o información adicional.*
- **Ruta B (Reutilización de Registro de Multiplicación):** Compartir los qubits auxiliares liberados de Kaliski con el multiplicador adyacente para ahorrar ~512 qubits transitorios.
- **Ruta C (Suma de Inversión Única):** Invertir una sola vez $w = dx^3$ y reconstruir $(Rx, Ry)$ mediante álgebra cerrada para ahorrar 1.8M Toffolis.


## Logros Actuales (Ruta B: Low-Scratch Mod-Add & CSWAP Boundary-Merge)
- Hemos implementado la versión final y robusta del sumador de constantes directas (`add_nbit_const_direct_fast`) utilizando una nueva compuerta base `z_if` añadida a la interfaz de Qrisp para garantizar la limpieza de fase cuántica sin utilizar qubits temporales.
- Redujimos el límite de iteraciones de Kaliski `pair1_iters` a 399 y `pair2_iters` a 399, lo cual pasa todos los tests de correctitud (9024 shots) con 0 basura de fase.
- **Fusión de CSWAP en Fronteras de Registros $(r,s)$ (`kal_cswap_rs_merge`):** Fusionamos y diferimos los CSWAPs del STEP 9 y el STEP 3 de la siguiente iteración basándonos en la paridad de la decisión, resolviendo la transición de la fase bulk a la genérica mediante la asignación y limpieza dinámica del qubit `frame`.
- Con esto logramos disminuir el recuento de Toffolis a **3,649,453** y los qubits a **2,710**, logrando un score de **9.89 × 10⁹** (rompiendo la barrera de 10¹⁰), un avance clave hacia la marca ideal. 


# Reglas de Optimización Cuántica para secp256k1 (AGY Rules)

Este archivo define las instrucciones operativas que el asistente de IA (Antigravity) debe seguir estrictamente al interactuar con este proyecto.

## 1. Contexto Académico e Histórico
- **El Reto:** Lograr la optimización máxima de la suma de puntos en la curva `secp256k1` (dominante en Shor para romper Bitcoin/Ethereum). Obtener una optimización significativa otorga un **5.0 definitivo** en la materia.
- **El Oráculo (ZKP):** El verificador público ZK de Google (Zenodo) actúa como una función de recompensa (Reward Function) interactiva: responde si un circuito es válido y devuelve su costo de Toffolis y qubits. Esto permite usar ciclos de optimización automática.
- **Frontera de Pareto de Google:**
  - *Low-Qubit:* 1,175 qubits, 2.7M Toffolis (Score: $3.2 \times 10^9$).
  - *Low-Gate:* 1,425 qubits, 2.1M Toffolis (Score: $3.0 \times 10^9$).
  - *Línea Base Inicial:* 2,715 qubits, 3,942,753 Toffolis (Score: $1.07 \times 10^{10}$).
  - *Nuestro Último Logro (Actual):* **2,310 qubits, 3,063,680 Toffolis** (Score: **$7.08 \times 10^{9}$**).

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
- **Ruta A (Eliminación de `m_hist`):** Reemplazar el vector de ~400 qubits con un qubit ancila local o cálculo on-the-fly. *Nota: Nuestros tests clásicos de determinismo del estado final revelaron que la información del bit menos significativo (LSB) se pierde en el desplazamiento a la derecha (Step 6) cuando add_f=0, haciendo que la recuperación de m_i en backward requiera pebbling de Bennett o información adicional.*
- **Ruta B (Reutilización de Registros):** Reutilizar los registros `tx` como `v_w` en Kaliski (Gouzien trick) ya implementado. Necesitamos buscar formas de no asignar 256 qubits en `tmp` durante `step4`.
- **Ruta C (Suma de Inversión Única):** Invertir una sola vez $w = dx^3$ y reconstruir $(Rx, Ry)$ mediante álgebra cerrada para ahorrar 1.8M Toffolis.
- **Ruta D (Reducción W-TRUNC):** El margen actual de truncación está en 32. Bajar a 16 causó fallos de CLASSICAL MISMATCH, por lo que 32 es el margen seguro actual.

## Logros Actuales
- **Modularización Total:** Todo el código en `mod.rs` fue separado en múltiples módulos (`cuccaro.rs`, `solinas.rs`, `kaliski_inv.rs`, etc.) manteniendo el mismo desempeño.
- **Pico de Qubits:** 2,310 (una reducción masiva de la línea base 2,715). Principalmente por reutilizar el estado clásico como variables transitorias en Kaliski (`v_w`).
- Hemos implementado la versión final y robusta del sumador de constantes directas (`add_nbit_const_direct_fast`) utilizando una nueva compuerta base `z_if`.
- Redujimos el límite de iteraciones de Kaliski `pair1_iters` a 399 y `pair2_iters` a 397 (según la rama actual), lo cual pasa todos los tests de correctitud.
- **Fusión de CSWAP en Fronteras de Registros $(r,s)$ (`kal_cswap_rs_merge`):** Fusionamos y diferimos los CSWAPs del STEP 9 y el STEP 3 de la siguiente iteración basándonos en la paridad de la decisión.
- Con esto logramos disminuir el recuento de Toffolis a **3,063,680** y los qubits a **2,310**, logrando un score de **7.08 × 10⁹** (estamos a ~2.4x de la mejor métrica de Google). 

## 4. Análisis de Viabilidad de Nuevas Rutas (Conclusiones de Probes)
Hemos analizado sistemáticamente las rutas restantes propuestas y todas están cerradas debido a regresiones críticas:
1. **Ruta A (Eliminación de `m_hist`):** **INVIABLE**. Las pruebas en `kaliski_classical_replay.rs` confirmaron que recomputar `m_i` en la fase reversa es imposible en el modelo estándar porque el LSB de `v_w` se pierde durante el corrimiento a la derecha (Step 6) cuando `add_f=0`.
2. **Coordenadas Proyectivas (`POINT_ADD_PROJECTIVE_N64_PROBE`):** **CERRADA**. El probe arrojó **9.43M Toffolis** y **7,321 qubits** (un aumento masivo de 5x Toffoli y 2.3x qubits frente al Affine de baseline).
3. **Ruta C (Estrategia C - Inversión Única):** **CERRADA**. El probe de un solo Kaliski requiere mantener vivos los operandos de entrada para desarmar la matemática reversa, lo que infló el pico a **4,505 qubits** y **71.8M operaciones**.
4. **Algoritmo EEA de Luo-Han (`POINT_ADD_LUOHAN_EEA_N64_PROBE`):** **CERRADA**. El probe arrojó **4.98M Toffolis** (casi 3x el baseline de Kaline de 1.7M) manteniendo el mismo pico de qubits.

*Veredicto Final:* El circuito actual de **3.06M Toffolis** y **2,310 qubits** es el diseño óptimo de Pareto alcanzable con la matemática modular y el algoritmo de Kaliski actuales.


import os

folder = '/home/emanuel/Documents/Universidad/Cripto/ellipticCurve/src/quantum_addition'

old_names = [
    "eea_classical_replay.rs",
    "eea_equiv.rs",
    "eea_jump.rs",
    "eea_linear_transform.rs",
    "halfgcd_coeff_decoder.rs",
    "halfgcd_live_pa.rs",
    "round158_halfgcd_splice_live.rs",
    "round185_halfgcd_fixed_depth64_pa.rs",
    "round218_b5_program.rs",
    "round218_b5_selector.rs",
    "round218_b5_transport.rs",
    "source_live_d1.rs",
    "unconditional_kal.rs",
    "venting.rs",
    "fermat_inv.rs",
    "by.rs",
    "microbench.rs"
]

for name in old_names:
    path = os.path.join(folder, name)
    if os.path.exists(path):
        os.remove(path)
        print(f"Removed old duplicate: {name}")

print("Cleanup of duplicates complete!")

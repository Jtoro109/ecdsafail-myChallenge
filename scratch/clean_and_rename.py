import os
import shutil

src_dir = '/home/emanuel/Documents/Universidad/Cripto/ellipticCurve/src'
old_folder = os.path.join(src_dir, 'ec_add')
new_folder = os.path.join(src_dir, 'quantum_addition')

# 1. Rename the main folder
if os.path.exists(old_folder):
    shutil.move(old_folder, new_folder)
    print(f"Renamed folder to {new_folder}")

# 2. Define file renaming mappings (only for active code files)
renames = {
    "eea_classical_replay.rs": "gcd_classical_emulator.rs",
    "eea_equiv.rs": "gcd_equivalence_check.rs",
    "eea_jump.rs": "gcd_jump_state.rs",
    "eea_linear_transform.rs": "gcd_linear_transform.rs",
    "halfgcd_coeff_decoder.rs": "gcd_coefficient_decoder.rs",
    "halfgcd_live_pa.rs": "gcd_live_addition.rs",
    "round158_halfgcd_splice_live.rs": "phase_gcd_splice_live.rs",
    "round185_halfgcd_fixed_depth64_pa.rs": "phase_gcd_fixed_depth.rs",
    "round218_b5_program.rs": "phase_b5_execution.rs",
    "round218_b5_selector.rs": "phase_b5_state_selector.rs",
    "round218_b5_transport.rs": "phase_b5_qubit_transport.rs",
    "source_live_d1.rs": "quantum_d1_data_source.rs",
    "unconditional_kal.rs": "unconditional_gcd_step.rs",
    "venting.rs": "quantum_venting_operations.rs",
    "fermat_inv.rs": "modular_inverse_fermat.rs",
    "by.rs": "coordinate_addition.rs",
    "microbench.rs": "performance_microbenchmarks.rs"
}

# Apply renames
for old_name, new_name in renames.items():
    old_path = os.path.join(new_folder, old_name)
    new_path = os.path.join(new_folder, new_name)
    if os.path.exists(old_path):
        os.rename(old_path, new_path)
        print(f"Renamed {old_name} -> {new_name}")

# 3. List of active files to preserve (including mod.rs and renamed ones)
active_files = {
    "mod.rs",
    "gcd_classical_emulator.rs",
    "gcd_equivalence_check.rs",
    "gcd_jump_state.rs",
    "gcd_linear_transform.rs",
    "gcd_coefficient_decoder.rs",
    "gcd_live_addition.rs",
    "phase_gcd_splice_live.rs",
    "phase_gcd_fixed_depth.rs",
    "phase_b5_execution.rs",
    "phase_b5_state_selector.rs",
    "phase_b5_qubit_transport.rs",
    "quantum_d1_data_source.rs",
    "unconditional_gcd_step.rs",
    "quantum_venting_operations.rs",
    "modular_inverse_fermat.rs",
    "coordinate_addition.rs",
    "performance_microbenchmarks.rs",
    # preserved test modules
    "coset_proto.rs",
    "kim_inv_circuit.rs",
    "kim_proto.rs",
    "luo_proto.rs",
    "primitive_costs.rs",
    "scratch600_frontier.rs",
    "single_inv_numeric.rs",
    "test_timeout.rs",
    "round125_jsf.rs"
}

# Remove unused/extraneous files
for item in os.listdir(new_folder):
    item_path = os.path.join(new_folder, item)
    if os.path.isfile(item_path) and item not in active_files:
        os.remove(item_path)
        print(f"Removed unused file: {item}")

print("Folder cleanup complete!")

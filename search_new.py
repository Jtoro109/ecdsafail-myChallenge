import os
import subprocess
import json

compare_bits_options = [63, 62, 61, 60, 59, 58, 57, 56, 55]
width_margin_options = [28, 27]
reroll_max = 20

print("Starting targeted grid search on the new codebase...")
print(f"Compare bits: {compare_bits_options}")
print(f"Width margins: {width_margin_options}")
print(f"Max Reroll iterations: {reroll_max}")
print("-" * 50)

best_score = float('inf')
best_config = None
log_file = "valid_configs_new.txt"

for width_margin in width_margin_options:
    for compare_bits in compare_bits_options:
        print(f"\nTesting WIDTH_MARGIN={width_margin}, COMPARE_BITS={compare_bits}...")
        
        for reroll in range(reroll_max + 1):
            env = os.environ.copy()
            env["DIALOG_GCD_WIDTH_MARGIN"] = str(width_margin)
            env["DIALOG_GCD_COMPARE_BITS"] = str(compare_bits)
            env["DIALOG_REROLL"] = str(reroll)
            
            # Build
            build_proc = subprocess.run(
                ["cargo", "run", "--release", "--bin", "build_circuit"], 
                env=env, 
                stdout=subprocess.DEVNULL, 
                stderr=subprocess.DEVNULL
            )
            
            if build_proc.returncode != 0:
                continue
                
            # Evaluate
            eval_proc = subprocess.run(
                ["cargo", "run", "--release", "--bin", "eval_circuit"],
                env=env,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True
            )
            
            if eval_proc.returncode == 0:
                try:
                    with open("score.json", "r") as f:
                        score_data = json.load(f)
                    score = score_data["score"]
                    toffoli = score_data["metrics"]["toffoli"]
                    qubits = score_data["metrics"]["qubits"]
                    
                    print(f"  -> SUCCESS! Reroll={reroll} | Toffolis={toffoli} | Qubits={qubits} | Score={score}")
                    
                    with open(log_file, "a") as lf:
                        lf.write(f"WIDTH_MARGIN={width_margin}, COMPARE_BITS={compare_bits}, REROLL={reroll} -> Toffolis={toffoli}, Qubits={qubits}, Score={score}\n")
                    
                    if score < best_score:
                        best_score = score
                        best_config = (width_margin, compare_bits, reroll, toffoli, qubits)
                        print(f"  *** NEW BEST SCORE: {best_score} ***")
                        # Stop searching rerolls for this pair
                        break
                except Exception as e:
                    pass
            else:
                out = eval_proc.stdout or ""
                if "correctness FAILED" in out or "CLASSICAL MISMATCH" in out or "PHASE GARBAGE" in out:
                    pass

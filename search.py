import os
import subprocess
import sys
import json

# Configurations to sweep
# Lower COMPARE_BITS and WIDTH_MARGIN reduce Toffoli count
compare_bits_options = [63, 62, 61, 60, 59, 58]
width_margin_options = [28, 27, 26, 25, 24]
reroll_max = 50

print("Starting grid search for optimal quantum circuit parameters...")
print(f"Compare bits: {compare_bits_options}")
print(f"Width margins: {width_margin_options}")
print(f"Max Reroll iterations: {reroll_max}")
print("-" * 50)

best_score = float('inf')
best_config = None

# Log file for valid configurations
log_file = "valid_configs.txt"

for width_margin in width_margin_options:
    for compare_bits in compare_bits_options:
        print(f"\nTesting WIDTH_MARGIN={width_margin}, COMPARE_BITS={compare_bits}...")
        
        for reroll in range(reroll_max + 1):
            # Setup environment
            env = os.environ.copy()
            env["DIALOG_GCD_WIDTH_MARGIN"] = str(width_margin)
            env["DIALOG_GCD_COMPARE_BITS"] = str(compare_bits)
            env["DIALOG_REROLL"] = str(reroll)
            
            # 1. Build the circuit
            build_cmd = ["cargo", "run", "--release", "--bin", "build_circuit"]
            build_proc = subprocess.run(
                build_cmd, 
                env=env, 
                stdout=subprocess.DEVNULL, 
                stderr=subprocess.DEVNULL
            )
            
            if build_proc.returncode != 0:
                print(f"  Reroll {reroll}: Build failed")
                continue
                
            # 2. Evaluate the circuit
            eval_cmd = ["cargo", "run", "--release", "--bin", "eval_circuit"]
            eval_proc = subprocess.run(
                eval_cmd,
                env=env,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True
            )
            
            # Check correctness
            if eval_proc.returncode == 0:
                # Read score from score.json
                try:
                    with open("score.json", "r") as f:
                        score_data = json.load(f)
                    score = score_data["score"]
                    toffoli = score_data["metrics"]["toffoli"]
                    qubits = score_data["metrics"]["qubits"]
                    
                    print(f"  -> SUCCESS! Reroll={reroll} | Toffolis={toffoli} | Qubits={qubits} | Score={score}")
                    
                    # Log to file
                    with open(log_file, "a") as lf:
                        lf.write(f"WIDTH_MARGIN={width_margin}, COMPARE_BITS={compare_bits}, REROLL={reroll} -> Toffolis={toffoli}, Qubits={qubits}, Score={score}\n")
                    
                    if score < best_score:
                        best_score = score
                        best_config = (width_margin, compare_bits, reroll, toffoli, qubits)
                        print(f"  *** NEW BEST SCORE: {best_score} ***")
                        
                        # Stop searching rerolls for this pair since we found a valid island
                        break
                except Exception as e:
                    print(f"  Reroll {reroll}: Success but error reading score.json: {e}")
            else:
                # Quick feedback on why it failed
                out = eval_proc.stdout or ""
                if "correctness FAILED" in out:
                    # Extract the failure message
                    fail_line = [line for line in out.split('\n') if "correctness FAILED" in line or "CLASSICAL MISMATCH" in line or "PHASE GARBAGE" in line]
                    fail_msg = fail_line[0].strip() if fail_line else "Correctness failed"
                    print(f"  Reroll {reroll:2d}: {fail_msg}")
                else:
                    print(f"  Reroll {reroll:2d}: Failed evaluation")

if best_config:
    wm, cb, rr, tof, qb = best_config
    print("\n" + "="*50)
    print("SEARCH COMPLETE!")
    print(f"Best Configuration:")
    print(f"  DIALOG_GCD_WIDTH_MARGIN = {wm}")
    print(f"  DIALOG_GCD_COMPARE_BITS = {cb}")
    print(f"  DIALOG_REROLL           = {rr}")
    print(f"Metrics:")
    print(f"  Toffolis                = {tof}")
    print(f"  Qubits                  = {qb}")
    print(f"  Score                   = {best_score}")
    print("="*50)
else:
    print("\nSearch finished. No valid configurations found in the swept range.")

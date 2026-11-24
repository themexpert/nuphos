#!/bin/bash
# Repro for #5031: run one device test N times on the booted emulator and count failures.
runs=$1 load=$2
test='ai.nuphos.android.BotDesktopFixtureDeviceTest#theKeyboardLeavesCancelAndSaveInView'
mkdir -p repro
if [ "$load" = true ]; then
  for _ in $(seq "$(nproc)"); do timeout 3h sh -c 'while :; do :; done' & done
fi
fail=0 skip=0
for i in $(seq "$runs"); do
  adb shell am instrument -w -e class "$test" ai.nuphos.android.test/androidx.test.runner.AndroidJUnitRunner > "repro/run-$i.txt" 2>&1
  if grep -q 'AssumptionViolated\|No soft keyboard' "repro/run-$i.txt"; then skip=$((skip + 1)); echo "run $i skipped"
  elif grep -q '^OK (1 test)' "repro/run-$i.txt"; then echo "run $i ok"
  else fail=$((fail + 1)); echo "run $i FAILED"; grep -m3 -E 'AssertionError|Exception' "repro/run-$i.txt"; fi
done
echo "REPRO SUMMARY runs=$runs failed=$fail skipped=$skip load=$load" | tee repro/summary.txt
[ "$load" = true ] && kill $(jobs -p) 2>/dev/null
exit 0

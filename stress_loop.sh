bin=`exec ./build.sh`

while true; do
    "$bin" "/dev/cdc-wdm1" & pid=$!
    sleep 5
    kill -INT "$pid" 2>/dev/null
    wait "$pid" 2>/dev/null
    sleep 1
done

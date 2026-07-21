bin=`exec ./build.sh $1`

while true; do
    "$bin" "/dev/cdc-wdm1" & pid=$!
    sleep 10
    kill -INT "$pid" 2>/dev/null
    wait "$pid" 2>/dev/null
    sleep 1
done

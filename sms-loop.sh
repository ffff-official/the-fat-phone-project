#!/bin/bash

USER_ID=$1
PASS=$2

while true; do
    DATE_MSG=$(date '+%Y-%m-%d %H:%M:%S')

    curl -G "https://smsapi.free-mobile.fr/sendmsg" \
        --data-urlencode "user=${USER_ID}" \
        --data-urlencode "pass=${PASS}" \
        --data-urlencode "msg=Hello World ! ${DATE_MSG}"

    sleep 3600
done

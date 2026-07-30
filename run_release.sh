# ./run.sh presentation | mobile_network | recv_mms | uim | voice_call_evt | answer_call | make_a_call | send_mms | at_at | nas
#                       | send_sms
bin=`exec ./build_release.sh $1`
"$bin" "unused_parameter"
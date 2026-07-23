# ./run.sh presentation | mobile_network | recv_mms | uim | voice_call_evt | answer_call | make_a_call
bin=`exec ./build.sh $1`
"$bin" "unused_parameter"

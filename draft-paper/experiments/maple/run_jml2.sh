filename=res.jsonl
dir=jml2
cd $dir && rm $filename -f && \
    git restore . && \
    clear && \
    cd .. && \
    python buggy_prepare.py $dir > $dir/classes.txt && \
    cd $dir && \
    \
    ../../../../target/release/llm-correct-features --config Ace.ecf --classes classes.txt --model claude-sonnet-4-5 | tee -a $filename

filename=res.jsonl
cd maple-recursive-eiffel/ && rm $filename && \
    clear && \
    cargo build --release -p llm-correct-features && \
    \
    git restore . && \
    clear && \
    \
    ../../../../target/release/llm-correct-features --config Ace.ecf --classes classes.txt --model "gpt-4.1-nano" | tee -a $filename
    \
    git restore . && \
    clear && \
    \
    ../../../../target/release/llm-correct-features --config Ace.ecf --classes classes.txt --model claude-sonnet-4-5 | tee -a $filename

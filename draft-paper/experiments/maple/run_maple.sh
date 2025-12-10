cd maple-recursive-eiffel/ &&
    clear && \
    cargo build --release -p llm-correct-features && \
    \
    git restore . && \
    clear && \
    \
    ../../../../target/release/llm-correct-features --config Ace.ecf --classes classes.txt | tee sonnet.jsonl && \
    \
    git restore . && \
    clear && \
    \
    ../../../../target/release/llm-correct-features --config Ace.ecf --classes classes.txt --model gpt-4o-mini | tee gpt4.jsonl

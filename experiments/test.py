import requests
import os
import json

TOKEN = os.getenv('CONSTRUCTOR_APP_API_TOKEN')
END_POINT='https://training.constructor.app/api/platform-kmapi/v1'
HEADERS = {
    'X-KM-AccessKey': f'Bearer {TOKEN}'
}

def any_knowledge_model_id():
    response = requests.get(f'{END_POINT}/knowledge-models', headers=HEADERS).json()
    id = response ['results'][0]['id']
    print(f"ID:\t{id}")
    return id


SYSTEM_MESSAGE={
    "role": "system",
    "content": """You are a coding assistant, expert in the Eiffel programming language and in formal methods.
You have extensive training in the usage of AutoProof, the static verifier of Eiffel.
You will receive a prompt in eiffel code with holes of the form <ADD_*>.
Write only model-based contracts, i.e. all qualified calls in all contract clauses will refer to the model of the target class and all unqualified calls in all contract clauses will refer to the model of the current class or its ancestors.
Respond with the same code, substituting the holes with valid eiffel code.
""",
    "name": "Coding assistant"}

SYSTEM_MESSAGE={
    "role": "system",
    "content": """You are a coding assistant, expert in the Eiffel programming language and in formal methods.
You have extensive training in the usage of AutoProof, the static verifier of Eiffel.
You will receive a prompt in eiffel code with holes of the form <ADD_*>.
Write only model-based contracts, i.e. all qualified calls in all contract clauses will refer to the model of the target class and all unqualified calls in all contract clauses will refer to the model of the current class or its ancestors.
Respond with the same code, substituting the holes with valid eiffel code.
""",
    "name": "Coding assistant"}

user_message={"role": "user", "content":
    """-- For the current class and its ancestors, the model is value: INTEGER
-- the model is implemented in Boogie.
-- For the argument other: NEW_INTEGER
--  the model is value: INTEGER
--    the model is terminal, no qualified call on it is allowed.
smaller (other: NEW_INTEGER): BOOLEAN
	do
		Result := value < other.value
	ensure
		Result = (value < other.value)
	end
"""}

user_message={"role": "user", "content":
    """-- For the current class and its ancestors, its model is: m: INTEGER
-- 	its model is terminal, no qualified call is allowed on this value.
-- For the argument arg: INTEGER
-- 	its model is terminal, no qualified call is allowed on this value.
max_arg_m(arg: INTEGER): INTEGER
	require
			<ADD_PRECONDITION_CLAUSES>
	do
			if m < arg then 
				Result := arg
			else
				Result := m
			end
	ensure
			<ADD_POSTCONDITION_CLAUSES>
	end
"""}

messages = [SYSTEM_MESSAGE, user_message]

def send_message_with_context(model, messages, stream="false"):
    data = {"model": model, "messages": messages, "stream":stream}
    response = requests.post(f'{END_POINT}/knowledge-models/{any_knowledge_model_id()}/chat/completions', headers=HEADERS, json=data)
    return response.json()

response = send_message_with_context("gemini-1.5-flash", messages)

val = json.dumps(response)

print(f'val: {val}')

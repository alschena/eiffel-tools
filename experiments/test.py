import requests
import os
import json

class Model:
	default_content_system_message = """You are a coding assistant, expert in the Eiffel programming language and in formal methods.
You have extensive training in the usage of AutoProof, the static verifier of Eiffel.
Write only model-based contracts, i.e. all qualified calls in all contract clauses will refer to the model of the target class and all unqualified calls in all contract clauses will refer to the model of the current class or its ancestors.
Respond with the same code, substituting the holes with valid eiffel code. """

	default_content_user_message = """note
	model: model_feature1, model_feature2
class
	MODEL_BASED_CONTRACTS

feature

	model_feature1: INTEGER
	model_feature2: INTEGER
	not_model_feature1: INTEGER
	not_model_feature2: INTEGER

	-- Add model-based contracts to the following feature, responding only in eiffel code.
	min: INTEGER
		require
			model_is_synced1: model_feature1 = not_model_feature1
			model_is_synced2: model_feature2 = not_model_feature2
		do
			if not_model_feature1 < not_model_feature2 then
				Result := not_model_feature1
			else 
				Result := not_model_feature2
		end
end """
	TOKEN = os.getenv('CONSTRUCTOR_APP_API_TOKEN')
	END_POINT='https://training.constructor.app/api/platform-kmapi/v1'
	HEADERS = {
	    'X-KM-AccessKey': f'Bearer {TOKEN}'
	}

	def _any_knowledge_model_id(self):
	    response = requests.get(f'{self.END_POINT}/knowledge-models', headers=self.HEADERS).json()
	    id = response ['results'][0]['id']
	    print(f"ID:\t{id}")
	    return id

	def _message(self, role, content):
		res = {
			'role': role,
			'content': content
		}
		return res

	def _named_message(self, role, content, name):
		message = self._message(role, content)
		message['name'] = name
		return message

	def _system_message(self, content):
		return self._named_message("system", content, "Coding assistant")

	def _user_message(self, content):
		return self._message("user", content)

	def __init__(self):
		self.km_id = self._any_knowledge_model_id()
		url = f'{self.END_POINT}/alive'
		alive_response = requests.post(url, headers = self.HEADERS)
		print(f'{alive_response}') 

	def messages(self,
	               system_message_content = default_content_system_message,
	               user_message_content = default_content_user_message):
		system_message = self._system_message(system_message_content)
		user_message = self._user_message(user_message_content)
		print(f'system_message: {system_message}')
		return [system_message, user_message]

	def send_messages(self, messages, model = "gemini-1.5-pro", stream="false"):
	    data = {"model": model, "messages": messages, "stream":stream}
	    print(f'data: {data}')
	    url = f'{self.END_POINT}/knowledge-models/{self.km_id}/chat/completions'
	    print(f'url: {url}')
	    response = requests.post(url, headers=self.HEADERS, json=data)
	    print(f'response: {response}')
	    return response.json()

model = Model()
messages = model.messages()
response = model.send_messages(messages)
val = json.dumps(response)

print(f'val: {val}')

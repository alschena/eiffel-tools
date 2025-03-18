import requests
import os
import json

class Model:
	default_content_system_message = """You are a coding assistant, expert in the Eiffel programming language and in formal methods.
You have extensive training in the usage of AutoProof, the static verifier of Eiffel.
Write only model-based contracts, i.e. all qualified calls in all contract clauses will refer to the model of the target class and all unqualified calls in all contract clauses will refer to the model of the current class or its ancestors.
Respond with the same code, substituting the holes with valid eiffel code. """

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

	def _messages(self,
					user_message_content,
					system_message_content = default_content_system_message):
		system_message = self._system_message(system_message_content)
		user_message = self._user_message(user_message_content)
		print(f'system_message: {system_message}')
		return [system_message, user_message]

	def __init__(self):
		self._km_id = self._any_knowledge_model_id()
		url = f'{self.END_POINT}/alive'
		alive_response = requests.post(url, headers = self.HEADERS)
		print(f'{alive_response}') 

	def upload_file(self, file_path):
	    # Prepare the file for uploading
	    files = {
	        'file': open(file_path, 'rb')
	    }

	    # Make the request to upload the file
	    response = requests.post(
	        f'{self.END_POINT}/knowledge-models/{self._km_id}/files',
	        headers=self.HEADERS,
	        files=files)

	    # Check the response from the API
	    if response.status_code == 200:
	        print("File uploaded successfully:", response.json())
	    else:
	        print("Failed to upload file. Status code:", response.status_code)
	        print("Response:", response.json())

	def list_documents(self):
		response = requests.get(f'{self.END_POINT}/knowledge-models/{self._km_id}/files',headers=self.HEADERS)
		response_json_fmt = response.json()
		print(f"{response_json_fmt}")
		for doc in response_json_fmt['results']:
		    print(f'{doc["filename"]}, {doc["in_use"]}, {doc["indexing_status"]}, {doc["id"]}')

	def query(self, user_message_content, system_message_content=default_content_system_message, model = "gemini-1.5-pro", stream="false"):
		messages = self._messages(user_message_content)
		data = {"model": model, "messages": messages, "stream":stream}
		print(f'Model: {model}')
		print(f'Input messages: {messages}')
		url = f'{self.END_POINT}/knowledge-models/{self._km_id}/chat/completions'
		response = requests.post(url, headers=self.HEADERS, json=data)
		print(f'response: {response}')
		return response.json()

default_content_user_message = """note
	description: "[
			Indexable containers with arbitrary bounds, whose elements are stored in a continuous memory area.
			Random access is constant time, but resizing requires memory reallocation and copying elements, and takes linear time.
			The logical size of array is the same as the physical size of the underlying memory area.
		]"
	author: "Nadia Polikarpova"
	revised_by: "Alexander Kogtenkov"
	model: sequence, lower_
	manual_inv: true
	false_guards: true

frozen class
	V_ARRAY [G]

inherit
	V_MUTABLE_SEQUENCE [G]
		redefine
			is_equal_,
			upper,
			fill,
			clear,
			is_model_equal
		end

create
	make,
	make_filled,
	copy_

feature {NONE} -- Initialization
-- 
-- 
feature -- Access

	-- Respond with the following feature, adding model-based contracts.
	-- Model-based contracts either refer to the model of current or the models of the arguments.
	-- INTEGER values can be used directly as they are themselves a model.
	subarray (l, u: INTEGER): V_ARRAY [G]
			-- Array consisting of elements of Current in index range [`l', `u'].
		note
			status: impure
		do
			create Result.make (l, u)
			check Result.inv end
			Result.copy_range (Current, l, u, Result.lower)
			check ∀ i: 1 |..| Result.sequence.count ¦ Result.sequence [i] = sequence [i - 1 + idx (l)] end
		end
"""
path_base2 = "/home/al_work/repos/reif/research/extension/autoproof/library/base/base2"

model = Model()
model.upload_file(f'{path_base2}/container/v_mutable_sequence.txt')
model.upload_file(f'{path_base2}/container/v_sequence.txt')
model.upload_file(f'{path_base2}/container/v_container.txt')
model.list_documents()
response = model.query(default_content_user_message)
val = json.dumps(response)

print(f'Output: {val}')

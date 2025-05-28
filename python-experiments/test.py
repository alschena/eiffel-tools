import requests
import os
import json

class Model:
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
					system_message_content):
		system_message = self._system_message(system_message_content)
		user_message = self._user_message(user_message_content)
		print(f'system_message: {system_message}')
		return [system_message, user_message]

	def __init__(self):
		self._km_id = self._any_knowledge_model_id()
		with open("/home/al_work/repos/eiffel-tools/experiments/system_message.txt", 'r') as system_message_file:
			self.system_message_content = system_message_file.read()

	def check_alive(self):
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
		results = response_json_fmt['results']
		print(f'{results}')
		ids = [doc["id"] for doc in results]
		return ids

	def _remove_file(self, file_id):
		url = f'{self.END_POINT}/knowledge-models/{self._km_id}/files/{file_id}'
		response = requests.delete(url, headers=self.HEADERS)
		print(f'reply status: {response.status_code}')

	def remove_all_files(self):
		list_document_ids = self.list_documents()
		for file_id in list_document_ids:
			self._remove_file(file_id)

	def query(self, prompt, model = "gemini-1.5-pro", stream="false"):
		messages = self._messages(prompt, self.system_message_content)
		data = {"model": model, "messages": messages, "stream":stream}
		print(f'Model: {model}')
		print(f'Input messages: {messages}')
		url = f'{self.END_POINT}/knowledge-models/{self._km_id}/chat/completions'
		response = requests.post(url, headers=self.HEADERS, json=data)
		print(f'response: {response}')
		return response.json()

	def query_from_file(self, file_path):
		with open(file_path, 'r') as file:
			return self.query(file.read())

path_base2 = "/home/al_work/repos/reif/research/extension/autoproof/library/base/base2"

model = Model()
# model.upload_file('/home/al_work/repos/reif/research/extension/autoproof/library/base/eve/simple_array.e')
# model.upload_file(f'{path_base2}/container/v_sequence.txt')
# model.upload_file(f'{path_base2}/container/v_container.txt')
# model.list_documents()
# response = model.query(default_content_user_message)
# val = json.dumps(response)

# print(f'Output: {val}')

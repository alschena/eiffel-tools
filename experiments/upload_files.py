import requests
import os
import json

TOKEN = os.getenv('CONSTRUCTOR_APP_API_TOKEN')
END_POINT='https://training.constructor.app/api/platform-kmapi/v1'
HEADERS = {
    'X-KM-AccessKey': f'Bearer {TOKEN}'
}

def create_model(name, description, shared_type):
    data = {
        "name": name,
        "description": description,
        "shared_type": shared_type
        }
    response = requests.post(f'{END_POINT}/knowledge-models', headers=HEADERS, json=data)
    return response.json() 

def any_knowledge_model_id():
    response = requests.get(f'{END_POINT}/knowledge-models', headers=HEADERS).json()
    if response ['total'] > 0:
        id = response ['results'][0]['id']
        print(f"ID:\t{id}")
        return id
    else:
        print('No knowledge model available')

KM_ID = any_knowledge_model_id()

def list_documents():
    response = requests.get(f'{END_POINT}/knowledge-models/{KM_ID}/files',headers=HEADERS)
    response_json_fmt = response.json()
    print(f"{response_json_fmt}")
    for doc in response_json_fmt['results']:
        print(f'{doc["filename"]}, {doc["in_use"]}, {doc["indexing_status"]}, {doc["id"]}')

base2_directory = '/home/al_work/repos/reif/research/extension/autoproof/library/base/base2'
files = [
    f'{base2_directory}/list/v_list.txt',
    f'{base2_directory}/list/v_linked_list_purged.txt',
    f'{base2_directory}/container/v_container.txt',
    f'{base2_directory}/container/v_mutable_sequence.txt',
    f'{base2_directory}/container/v_sequence.txt',
]

def upload_file(file_path):
    # Prepare the file for uploading
    files = {
        'file': open(file_path, 'rb')
    }

    # Make the request to upload the file
    response = requests.post(
        f'{END_POINT}/knowledge-models/{KM_ID}/files',
        headers=HEADERS,
        files=files)

    # Check the response from the API
    if response.status_code == 200:
        print("File uploaded successfully:", response.json())
    else:
        print("Failed to upload file. Status code:", response.status_code)
        print("Response:", response.json())

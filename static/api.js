function apiCall(method, endpoint, callback, data) {
  let xhr = new XMLHttpRequest();

  xhr.onreadystatechange = function() {
    if (xhr.readyState === XMLHttpRequest.DONE && xhr.status === 200) {
      callback(JSON.parse(xhr.responseText));
    }
  };

  xhr.open(method, "/api/" + endpoint);

  if (data) {
    xhr.send(JSON.stringify(data));
  } else {
    xhr.send();
  }
}

function lookupNumber(number, callback) {
  apiCall("POST", "entry/" + number, callback);
}

function getPublicEntries(callback) {
  apiCall("GET", "entries", callback);
}

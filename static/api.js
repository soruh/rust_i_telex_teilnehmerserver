function api_call(method, endpoint, callback, data) {
  let xhr = new XMLHttpRequest();

  xhr.responseType = "json";
  xhr.onreadystatechange = function() {
    if (xhr.readyState === XMLHttpRequest.DONE) {
      if (xhr.status === 200) {
        if (typeof callback === "function") callback(xhr.response);
      } else {
        var err = xhr.response;
        console.error(
          "API call: " + method + " to " + endpoint + " failed: " + xhr.response
        );

        if (err) {
          alert("Server Error: " + err);
          throw err;
        }
      }
    }
  };

  xhr.open(method, "/api/" + endpoint);

  if (data) {
    xhr.send(JSON.stringify(data));
  } else {
    xhr.send();
  }
}

function inferDeletedField(entry) {
  entry.disabled = Boolean(entry.flags & 2);
  return entry;
}

function get_entry(number, callback) {
  api_call("GET", "entry/" + number, res => callback(inferDeletedField(res)));
}

function get_entries(callback) {
  api_call("GET", "entries", res => callback(res.map(inferDeletedField)));
}

function login(password, callback) {
  api_call("POST", "login", callback, { password });
}

function logout(callback) {
  api_call("GET", "logout", callback);
}

function logged_in(callback) {
  api_call("GET", "logged-in", callback);
}

function update_entry(number, entry, callback) {
  api_call("POST", "entry/" + number, callback, entry);
}

function new_entry(entry, callback) {
  api_call("POST", "entry", callback, entry);
}

function reset_pin(number, callback) {
  api_call("GET", "reset_pin/" + number, callback);
}

function load_localizations(language, callback) {
  api_call("GET", "localizations/" + language, callback);
}

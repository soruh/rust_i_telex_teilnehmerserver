function apiCall(method, endpoint, callback, data) {
  let xhr = new XMLHttpRequest();

  xhr.responseType = "json";
  xhr.onreadystatechange = function() {
    if (xhr.readyState === XMLHttpRequest.DONE) {
      if (xhr.status === 200) {
        if (typeof callback === "function") callback(null, xhr.response);
      } else {
        console.error("API call failed: " + xhr.response);
        if (typeof callback === "function") callback(xhr.response, null);
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

function lookupNumber(number, callback) {
  apiCall("GET", "entry/" + number, callback);
}

function getPublicEntries(callback) {
  apiCall("GET", "entries", callback);
}

function login(password, callback) {
  apiCall("POST", "login", callback, { password });
}

function logged_in(callback) {
  apiCall("GET", "logged-in", callback);
}

function stringifyExtension(ext) {
  if (ext === 0) return null;
  if (ext >= 1 && ext <= 99) return ext.toString().padStart(2, "0");
  if (ext === 100) return "00";
  if (ext > 100 && ext < 110) return (ext - 100).toString();
  if (ext === 110) return "0";
  if (ext > 110 || ext < 0) return null; // invalid
}

function parseExtension(ext) {
  if (!ext) return 0;
  if (isNaN(parseInt(ext))) return 0;
  if (ext === "0") return 110;
  if (ext === "00") return 100;
  if (ext.length === 1) return parseInt(ext) + 100;

  let res = parseInt(ext);
  if (isNaN(res)) return null;
  return res;
}

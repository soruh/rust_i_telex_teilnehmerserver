document.addEventListener("DOMContentLoaded", function() {
  start();
});

logged_in(logged_in => {
  is_logged_in = logged_in;
  start();
});
load_localizations("de", localizations => {
  locs = localizations;
  start();
});

var n = 3;
function start() {
  if (--n <= 0) {
    if (typeof main === "function") {
      main(is_logged_in);
    } else {
      alert("error: no main function found");
    }

    n = Infinity;
  }
}

ITELEX_TIMESTAMP_DELTA = 60 * 60 * 24 * (365 * 70 + 17);
function formatValue(key, value) {
  switch (key) {
    case "extension":
      value = stringifyExtension(value);
      break;

    case "disabled":
      value = value ? locs.yes : locs.no;
      break;

    case "client_type":
      value = locs.client_types[value];
      break;

    case "timestamp":
      value = new Date((value - ITELEX_TIMESTAMP_DELTA) * 1000).toLocaleString(
        undefined,
        {
          year: "2-digit",
          month: "2-digit",
          day: "2-digit",
          hour: "2-digit",
          minute: "2-digit"
        }
      );
      break;
  }

  return value;
}

function stringifyExtension(ext) {
  if (ext === 0) return "-";
  if (ext >= 1 && ext <= 99) return ext.toString().padStart(2, "0");
  if (ext === 100) return "00";
  if (ext > 100 && ext < 110) return (ext - 100).toString();
  if (ext === 110) return "0";
  if (ext > 110 || ext < 0) {
    console.error("go invalid extension code", ext);
    return "-"; // invalid
  }
}

function parseExtension(ext) {
  if (!ext || ext === "-") return 0;
  if (isNaN(parseInt(ext))) return 0;
  if (ext === "0") return 110;
  if (ext === "00") return 100;
  if (ext.length === 1) return parseInt(ext) + 100;

  let res = parseInt(ext);
  if (isNaN(res)) return null;
  return res;
}

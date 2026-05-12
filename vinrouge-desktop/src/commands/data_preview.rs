use tauri::{AppHandle, Emitter, WebviewWindowBuilder};

/// Open a new window to display data preview with multiple tables, tab-switching, and sorting.
#[tauri::command]
pub async fn open_data_preview_window(app: AppHandle, data: serde_json::Value) -> Result<(), String> {
    // Accept multi-table format: { "tables": [{ "name", "columns", "rows" }, ...] }
    let tables_val = data["tables"].as_array().ok_or("Missing tables in data")?;

    // Serialize table data for embedding as JSON in the HTML
    let tables_json = serde_json::to_string(tables_val)
        .unwrap_or_default()
        .replace("</script>", "<\\/script>");

    // Collect (name, row_count) for the tab bar
    let tab_info: Vec<(String, usize)> = tables_val
        .iter()
        .map(|t| {
            let name = t["name"].as_str().unwrap_or("Table").to_string();
            let count = t["rows"].as_array().map(|r| r.len()).unwrap_or(0);
            (name, count)
        })
        .collect();

    let first_n = tab_info.first().map(|(_, n)| *n).unwrap_or(0);

    let mut html = String::new();
    html.push_str(
        r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>Data Preview</title>
<style>
*{box-sizing:border-box}
body{margin:0;padding:0;background:#141414;color:#c0c0c0;
font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif;
display:flex;flex-direction:column;height:100vh;overflow:hidden}
.topbar{display:flex;align-items:center;padding:5px 10px;border-bottom:0.5px solid #2a2a2a;
flex-shrink:0;gap:6px;background:#161616;overflow-x:auto}
.tab{background:none;border:0.5px solid transparent;border-radius:4px;padding:3px 8px;
font-size:11px;color:#555;cursor:pointer;display:flex;align-items:center;gap:5px;white-space:nowrap}
.tab:hover{color:#888;background:#1e1e1e}
.tab.active{color:#c0c0c0;background:#1e1e1e;border-color:#333}
.tab-count{background:#252525;border-radius:99px;padding:1px 5px;font-size:9px;color:#555}
.tab.active .tab-count{color:#888}
.pill{background:#1e1e1e;border:0.5px solid #333;border-radius:99px;padding:2px 8px;
font-size:10px;color:#666;flex-shrink:0;margin-left:auto}
.table-wrap{flex:1;overflow:auto}
table{width:100%;border-collapse:collapse;table-layout:auto}
th.lbl{background:#1a1a1a;padding:4px 7px;font-size:10px;font-weight:400;color:#444;
border-bottom:0.5px solid #2a2a2a;border-right:0.5px solid #222;text-align:center;
position:sticky;top:0;z-index:2}
th.lbl.corner{background:#111}
th.fld{background:#1a1a1a;padding:4px 7px;font-size:10px;font-weight:400;color:#666;
border-bottom:0.5px solid #282828;border-right:0.5px solid #222;text-align:left;
white-space:nowrap;position:sticky;top:23px;z-index:2;cursor:pointer;user-select:none}
th.fld:hover{color:#999;background:#1e1e1e}
th.fld.sort-asc,th.fld.sort-desc{color:#9fe1cb}
td{padding:5px 7px;border-bottom:0.5px solid #1e1e1e;border-right:0.5px solid #1a1a1a;
font-size:11px;color:#c0c0c0;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;max-width:160px}
td.rn{background:#181818;color:#333;font-size:10px;text-align:center;padding:5px 3px;
border-right:0.5px solid #222;width:28px;min-width:28px}
tbody tr:hover td{background:#1c1c1c}
</style>
<script>
const TABLES = "#,
    );
    html.push_str(&tables_json);
    html.push_str(
        r#";
var T=0,SC=null,SA=true;
var ROW_H=28,OVERSCAN=20,PENDING=false;

function L(i){return i<26?String.fromCharCode(65+i):String.fromCharCode(64+Math.floor(i/26))+String.fromCharCode(65+(i%26));}
function E(s){var v=s==null?'':String(s);return v.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');}

// Walk up from a node (element or text) to find an ancestor with the given tag name.
function upTag(node,tag){
  while(node){
    if(node.nodeType===1&&node.tagName===tag){return node;}
    node=node.parentElement||node.parentNode;
  }
  return null;
}

function render(){
  var t=TABLES[T];
  var rows=t.rows.slice();
  if(SC!==null){
    rows.sort(function(a,b){
      var va=a[SC]==null?'':String(a[SC]);
      var vb=b[SC]==null?'':String(b[SC]);
      var na=parseFloat(va);
      var nb=parseFloat(vb);
      var c=(!isNaN(na)&&!isNaN(nb))?(na-nb):String(va).toLowerCase().localeCompare(String(vb).toLowerCase());
      return SA?c:-c;
    });
  }

  var wrap=document.getElementById('g');
  var st=wrap.scrollTop;
  var vh=wrap.clientHeight||wrap.offsetHeight||500;
  var total=rows.length;
  var start=Math.max(0,Math.floor(st/ROW_H)-OVERSCAN);
  var end=Math.min(total,Math.ceil((st+vh)/ROW_H)+OVERSCAN);

  var h='<table><thead><tr><th class="lbl corner"></th>';
  for(var i=0;i<t.columns.length;i++){h+='<th class="lbl">'+L(i)+'</th>';}
  h+='</tr><tr><th class="lbl" style="background:#111"></th>';
  for(var i=0;i<t.columns.length;i++){
    var cls='fld'+(SC===i?(SA?' sort-asc':' sort-desc'):'');
    var arr=SC===i?(SA?' ↑':' ↓'):'';
    h+='<th class="'+cls+'" data-ci="'+i+'">'+E(t.columns[i])+arr+'</th>';
  }
  h+='</tr></thead><tbody>';

  if(start>0){
    h+='<tr style="height:'+(start*ROW_H)+'px"><td colspan="'+(t.columns.length+1)+'"></td></tr>';
  }

  for(var ri=start;ri<end;ri++){
    h+='<tr><td class="rn">'+(ri+1)+'</td>';
    var row=rows[ri];
    for(var ci=0;ci<t.columns.length;ci++){
      h+='<td>'+E(row[ci])+'</td>';
    }
    h+='</tr>';
  }

  if(end<total){
    h+='<tr style="height:'+((total-end)*ROW_H)+'px"><td colspan="'+(t.columns.length+1)+'"></td></tr>';
  }

  h+='</tbody></table>';
  wrap.innerHTML=h;
  document.getElementById('rc').textContent=total+' rows';
}

function sw(i){T=i;SC=null;SA=true;updateTabs();render();}
function updateTabs(){var tabs=document.querySelectorAll('.tab');for(var j=0;j<tabs.length;j++){tabs[j].classList.toggle('active',j===T);}}
function srt(ci){
  if(SC===ci){if(SA){SA=false;}else{SC=null;SA=true;}}else{SC=ci;SA=true;}
  var wrap=document.getElementById('g');
  if(wrap){wrap.scrollTop=0;}
  render();
}

document.addEventListener('DOMContentLoaded',function(){
  render();
  document.getElementById('g').addEventListener('click',function(e){
    try{
      var th=upTag(e.target,'TH');
      if(th&&th.hasAttribute('data-ci')){
        srt(parseInt(th.getAttribute('data-ci'),10));
      }
    }catch(err){console.error('sort click error',err);}
  });
  document.getElementById('g').addEventListener('scroll',function(){
    if(!PENDING){
      PENDING=true;
      requestAnimationFrame(function(){PENDING=false;render();});
    }
  });
  document.querySelector('.topbar').addEventListener('click',function(e){
    try{
      var btn=upTag(e.target,'BUTTON');
      if(btn&&btn.hasAttribute('data-ti')){sw(parseInt(btn.getAttribute('data-ti'),10));}
    }catch(err){console.error('tab click error',err);}
  });
});
</script></head><body>
<div class="topbar">
"#,
    );

    for (i, (name, count)) in tab_info.iter().enumerate() {
        let safe = name
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        let active = if i == 0 { " active" } else { "" };
        html.push_str(&format!(
            r#"<button class="tab{active}" data-ti="{i}">{safe}<span class="tab-count">{count}</span></button>"#
        ));
    }

    html.push_str(&format!(
        r#"<span id="rc" class="pill">{first_n} rows</span></div>
<div id="g" class="table-wrap"></div></body></html>"#
    ));

    // Write HTML to a temporary file
    use std::io::Write;
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join("vinrouge_data_preview.html");

    std::fs::File::create(&file_path)
        .and_then(|mut f| f.write_all(html.as_bytes()))
        .map_err(|e| format!("Failed to write temp file: {e}"))?;

    // Create window pointing to the temp file
    #[cfg(target_os = "windows")]
    let file_url = format!("file:///{}", file_path.display().to_string().replace('\\', "/"));
    #[cfg(not(target_os = "windows"))]
    let file_url = format!("file://{}", file_path.display());
    let window = WebviewWindowBuilder::new(
        &app,
        "data-preview",
        tauri::WebviewUrl::External(file_url.parse().unwrap()),
    )
    .title("Data Preview")
    .inner_size(1200.0, 800.0)
    .build()
    .map_err(|e| format!("Failed to create window: {e}"))?;

    // Listen for window close event and emit to main window
    let app_handle = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            let _ = app_handle.emit("data-preview-closed", ());
        }
    });

    Ok(())
}

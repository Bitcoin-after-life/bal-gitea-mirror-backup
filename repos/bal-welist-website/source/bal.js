var chain = 'bitcoin';
var page = 0;
var limit = 100;
  
const urlParams = new URLSearchParams(window.location.search);
if (urlParams.has('page')) {
  page =urlParams.get('page',0);
}
if (urlParams.has('limit')){
  limit = urlParams.get('limit',100);
}
if (urlParams.has('chain')){
  chain = urlParams.get('chain','bitcoin');
}


function get_chain(){
  var e = document.getElementById('chain')
  const idx = e.selectedIndex;
  if (idx == 0){
    return 'bitcoin';
  }
  else if (idx == 1) {
    return 'testnet';
  }
  else if (idx == 2) {
    return 'regtest';
  }
}
function downloadLink(data,options) {
  const jsonString = JSON.stringify(data);
  const blob = new Blob([jsonString], { type: 'application/json;charset=utf-8' });

  const urlCreator = URL.createObjectURL(blob);
  const a = document.getElementById(options.elementName);
  a.href = urlCreator;
  a.setAttribute(options.linkText, options.fileName);
}

async function fetchDataAndPopulateTable(url,params,fnPopulateTable,downloadOpt,onError){
  try {
    //params.append("page",page);
    //params.append("limit",limit);
    //const response = await fetch('/wetime/'+get_chain()+"?"+params.toString());
    var realurl = url
    if (params){
      params.append("page",page);
      params.append("limit",limit);
      realurl+="?"+params.toString();
    }

    const response = await fetch(realurl);
    if (!response.ok) {
      console.log("error",response);
      throw new Error(`HTTP error! status: ${response.status}`);
    }

    const data = await response.json();
    if (downloadOpt){
      downloadLink(data,downloadOpt);
    }
    var paginator=fnPopulateTable(data);
    make_paginator(paginator);
  } catch (error) {
    console.error('Error fetching data:', error);
    if (onError){
      onError(error);
    }
  }
  function createlink(html,dest){
    const link = document.createElement("a");
    link.href=dest;
    link.innerHTML=html
    return link;
  }
  function make_paginator(){
    document.getElementById("chainth").innerHTML = chain;
    const pag = document.getElementById("paginator");
    pag.innerHTML = "";
    const currentPage = paginator && paginator.page != null
      ? paginator.page
      : (Number(page) || 0);
    const prevPage = paginator && paginator.prev != null
      ? paginator.prev
      : Math.max(currentPage - 1, 0);
    const nextPage = paginator && paginator.next != null
      ? paginator.next
      : currentPage + 1;
    const hasPrev = paginator && paginator.hasPrev != null
      ? paginator.hasPrev
      : currentPage > 0;
    const hasNext = paginator && paginator.hasNext != null
      ? paginator.hasNext
      : true;
    // prev (a plain, dimmed arrow on the first page), "page N", next
    if (hasPrev) {
      pag.appendChild(createlink("&lsaquo;","?page="+prevPage));
    } else {
      const off = document.createElement("span");
      off.className = "pg-off"; off.innerHTML = "&lsaquo;";
      pag.appendChild(off);
    }
    const lbl = document.createElement("span");
    lbl.className = "pg-label"; lbl.textContent = "page " + currentPage;
    pag.appendChild(lbl);
    if (hasNext) {
      pag.appendChild(createlink("&rsaquo;","?page="+nextPage));
    } else {
      const off = document.createElement("span");
      off.className = "pg-off"; off.innerHTML = "&rsaquo;";
      pag.appendChild(off);
    }
  }
}


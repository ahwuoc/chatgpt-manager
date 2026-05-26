import React from "react";
import { Modal, Input } from "antd";

export default function Modals({
  showImportModal,
  setShowImportModal,
  bulkText,
  setBulkText,
  handleImportBulk,
}) {
  return (
    <>
      {/* Modal: Import Bulk */}
      <Modal
        title="Nhập tài khoản hàng loạt"
        open={showImportModal}
        onCancel={() => {
          setShowImportModal(false);
          setBulkText("");
        }}
        onOk={handleImportBulk}
        okText="Phân tích & Import"
        cancelText="Hủy"
        width={700}
        className="antd-custom-modal"
        okButtonProps={{ className: "rounded-xl bg-amber-500 hover:bg-amber-400 border-none text-slate-950 font-semibold h-10 px-5" }}
        cancelButtonProps={{ className: "rounded-xl border-white/10 text-slate-300 h-10" }}
      >
        <div className="space-y-4 py-3">
          <p className="text-xs text-slate-400 leading-normal">
            Dán danh sách tài khoản Hotmail của bạn, mỗi dòng một tài khoản theo định dạng:<br />
            <code className="text-amber-400 font-mono block bg-black/35 p-2 rounded-lg mt-1.5 border border-white/5">email|password|hotmail_refresh_token(tùy chọn)|accountId(tùy chọn)</code>
          </p>
          <Input.TextArea
            placeholder="email|password&#10;email|password|hotmail_refresh_token|accountId"
            value={bulkText}
            onChange={(e) => setBulkText(e.target.value)}
            rows={12}
            className="font-mono text-xs leading-relaxed rounded-xl bg-white/5 border-white/10 text-white"
          />
        </div>
      </Modal>
    </>
  );
}

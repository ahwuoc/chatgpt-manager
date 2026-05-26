import React from "react";
import AccountTable from "../components/AccountTable";

export default function AccountsPage({ consoleState }) {
  return (
    <AccountTable
      accounts={consoleState.accounts}
      filteredAccounts={consoleState.filteredAccounts}
      status={consoleState.status}
      runningEmails={consoleState.runningEmails}
      runMode={consoleState.runMode}
      setRunMode={consoleState.setRunMode}
      threadCount={consoleState.threadCount}
      setThreadCount={consoleState.setThreadCount}
      selectedEmails={consoleState.selectedEmails}
      setSelectedEmails={consoleState.setSelectedEmails}
      activeStatusTab={consoleState.activeStatusTab}
      setActiveStatusTab={consoleState.setActiveStatusTab}
      subFilter={consoleState.subFilter}
      setSubFilter={consoleState.setSubFilter}
      searchQuery={consoleState.searchQuery}
      setSearchQuery={consoleState.setSearchQuery}
      countStats={consoleState.countStats}
      countSubFilter={consoleState.countSubFilter}
      loadData={consoleState.loadData}
      setShowImportModal={consoleState.setShowImportModal}
      getNextSmartWorkflow={consoleState.getNextSmartWorkflow}
      handleStartAutomation={consoleState.handleStartAutomation}
      handleCopyToken={consoleState.handleCopyToken}
      triggerGetOTP={consoleState.triggerGetOTP}
      markAccountStatus={consoleState.markAccountStatus}
      markMultipleAccountsStatus={consoleState.markMultipleAccountsStatus}
      WORKFLOW_STEPS={consoleState.WORKFLOW_STEPS}
      isScanningPlusMail={consoleState.isScanningPlusMail}
      handleScanPlusMailStatus={consoleState.handleScanPlusMailStatus}
      isImporting9Router={consoleState.isImporting9Router}
      handleImportPlusRealTo9Router={consoleState.handleImportPlusRealTo9Router}
      isExporting9Router={consoleState.isExporting9Router}
      handleExportSelected9RouterScripts={consoleState.handleExportSelected9RouterScripts}
      last9RouterExportDir={consoleState.last9RouterExportDir}
      handleOpenFolder={consoleState.handleOpenFolder}
    />
  );
}

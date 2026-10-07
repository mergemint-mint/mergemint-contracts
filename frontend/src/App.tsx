import React, { Suspense, useEffect, lazy } from 'react';
import { BrowserRouter, Link, Route, Routes, useLocation } from 'react-router-dom';
import { WalletProvider, useWallet } from './lib/WalletContext';
import { WalletConnectButton } from './components/WalletConnectButton';
import { NetworkMismatchBanner } from './components/NetworkMismatchBanner';
import { BountyList } from './pages/BountyList';
import { BountyDetail } from './pages/BountyDetail';
import { CreateBounty } from './pages/CreateBounty';
import { ContributorProfile } from './pages/ContributorProfile';
import { Leaderboard } from './pages/Leaderboard';
import { BountyErrorBoundary } from './components/BountyErrorBoundary';
import { useTranslation } from './i18n';

function Nav() {
  const location = useLocation();
  const { address, connect, clearError } = useWallet();
  const { t } = useTranslation();

  // A prior connect() failure otherwise stays visible until the next
  // connect() attempt, even after navigating away (issue #508).
  useEffect(() => {
    clearError();
  }, [location.pathname, clearError]);

  return (
    <nav>
      <Link to="/">Bounties</Link>
      <Link to="/create">Create Bounty</Link>
      <Link to="/leaderboard">Leaderboard</Link>
      <Link to="/create" role="button">Create Bounty</Link>
      <WalletConnectButton address={address} onConnect={connect} />
    </nav>
  );
}

export default function App() {
  return (
    <WalletProvider>
      <BrowserRouter>
        <Nav />
        <NetworkMismatchBanner />
        <Routes>
          <Route path="/" element={<BountyList />} />
          <Route path="/bounties/:id" element={<BountyDetail />} />
          <Route path="/create" element={<CreateBounty />} />
          <Route path="/contributors/:address" element={<ContributorProfile />} />
          <Route path="/leaderboard" element={<Leaderboard />} />
          <Route
            path="/"
            element={
              <BountyErrorBoundary>
                <BountyList />
              </BountyErrorBoundary>
            }
          />
          <Route
            path="/bounties/:id"
            element={
              <BountyErrorBoundary>
                <BountyDetail />
              </BountyErrorBoundary>
            }
          />
          <Route
            path="/create"
            element={
              <BountyErrorBoundary>
                <CreateBounty />
              </BountyErrorBoundary>
            }
          />
          <Route
            path="/contributors/:address"
            element={
              <BountyErrorBoundary>
                <ContributorProfile />
              </BountyErrorBoundary>
            }
          />
        </Routes>
      </BrowserRouter>
    </WalletProvider>
  );
}
